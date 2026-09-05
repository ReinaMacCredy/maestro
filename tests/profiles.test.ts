import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import {
  materializeProfiles,
  parseProfile,
  planProfileRenders,
  removeRenderedProfiles,
  renderedProfilePath,
} from "../src/plugins/profiles.ts";
import { prepareInstallFixture, runCli, withFixture, type Fixture } from "./helpers.ts";

const everyKey = `---
harness: claude
model: opus
effort: high
permission: acceptEdits
autocompact: 250000
disallowed_tools: [Write, Edit, NotebookEdit]
description: every key set
---
Role: every key.

The body is the mandate.
`;

async function writeProfile(directory: string, name: string, text: string): Promise<string> {
  await mkdir(directory, { recursive: true });
  const path = join(directory, `${name}.md`);
  await writeFile(path, text);
  return path;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

async function claudeBody(fixture: Fixture, name: string): Promise<string> {
  const text = await readFile(renderedProfilePath(fixture.home, "claude", name), "utf8");
  return text.slice(text.indexOf("\n---\n") + "\n---\n".length);
}

test("profile-parse: every key parses, each malformed profile fails install naming the file and key, repo shadows home shadows shipped (red 1)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const repoProfiles = join(fixture.repo, ".maestro", "profiles");
    const homeProfiles = join(fixture.home, "maestro", "profiles");
    await writeProfile(repoProfiles, "every", everyKey);

    const installed = await runCli(fixture, ["install"], { PATH: path });
    expect(installed.exitCode).toBe(0);
    const rendered = await readFile(join(fixture.home, ".claude", "agents", "maestro-every.md"), "utf8");
    expect(rendered).toContain("name: maestro-every\n");
    expect(rendered).toContain("model: opus\n");
    expect(rendered).toContain("effort: high\n");
    expect(rendered).toContain("permissionMode: acceptEdits\n");
    expect(rendered).toContain("disallowedTools: Write, Edit, NotebookEdit\n");
    expect(await claudeBody(fixture, "every")).toBe("\nRole: every key.\n\nThe body is the mandate.\n");

    const malformed: Array<[string, string, string]> = [
      ["unknown", "harness: claude\nmodel: default\ncolour: red\n", "colour"],
      ["badharness", "harness: gemini\nmodel: default\n", "harness"],
      ["badeffort", "harness: codex\nmodel: default\neffort: extreme\n", "effort"],
    ];
    for (const [name, frontmatter, key] of malformed) {
      const written = await writeProfile(repoProfiles, name, `---\n${frontmatter}---\nRole: ${name}.\n`);
      const refused = await runCli(fixture, ["install"], { PATH: path });
      expect(refused.exitCode).toBe(1);
      const error = failure(refused.stderr);
      expect(error.code).toBe("INVALID_PROFILE");
      expect(error.message).toContain(written);
      expect(error.message).toContain(key);
      await writeFile(written, everyKey);
    }
    const bodiless = await writeProfile(repoProfiles, "bodiless", "---\nharness: claude\nmodel: default\n---\n\n");
    const refused = await runCli(fixture, ["install"], { PATH: path });
    expect(refused.exitCode).toBe(1);
    expect(failure(refused.stderr).code).toBe("INVALID_PROFILE");
    expect(failure(refused.stderr).message).toContain(bodiless);
    expect(failure(refused.stderr).message).toContain("body");
    for (const name of ["unknown", "badharness", "badeffort", "bodiless"]) {
      await rm(join(repoProfiles, `${name}.md`), { force: true });
    }

    // Shadowing: the shipped lead is codex; a home copy wins over it and a repo copy wins over home.
    await writeProfile(homeProfiles, "lead", "---\nharness: claude\nmodel: default\n---\nRole: home lead.\n");
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await claudeBody(fixture, "lead")).toContain("Role: home lead.");
    expect(existsSync(join(fixture.home, ".codex", "maestro-lead.config.toml"))).toBe(true);
    await writeProfile(repoProfiles, "lead", "---\nharness: claude\nmodel: default\n---\nRole: repo lead.\n");
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await claudeBody(fixture, "lead")).toContain("Role: repo lead.");
    expect(await claudeBody(fixture, "lead")).not.toContain("home lead");
  });
}, 90_000);

test("package-json: no dependencies key (red 10, A5)", async () => {
  const packageJson = JSON.parse(
    await readFile(join(import.meta.dir, "..", "package.json"), "utf8"),
  ) as Record<string, unknown>;
  expect("dependencies" in packageJson).toBe(false);
});

test("profile-no-question-tool: every shipped profile renders disallowedTools: AskUserQuestion for Claude, its composed peer render too, and the Codex TOMLs never carry it (owner ruling 2026-09-05)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const shipped = join(import.meta.dir, "..", "src", "plugins", "resources", "profiles");
    const names = (await readdir(shipped)).filter((entry) => entry.endsWith(".md")).map((entry) => entry.slice(0, -3));
    expect(names.length).toBeGreaterThan(0);
    for (const name of names) {
      const renderedNames = name.startsWith("peer-") || ["team-supervisor", "lead", "peer"].includes(name)
        ? [name]
        : [name, `peer-${name}`];
      for (const renderedName of renderedNames) {
        const claude = await readFile(renderedProfilePath(fixture.home, "claude", renderedName), "utf8");
        const frontmatter = claude.slice(0, claude.indexOf("\n---\n", 4));
        const line = frontmatter.split("\n").find((candidate) => candidate.startsWith("disallowedTools: "));
        expect({ renderedName, line }).toEqual({ renderedName, line: expect.stringContaining("AskUserQuestion") });
        for (const toml of [
          join(fixture.home, ".codex", `maestro-${renderedName}.config.toml`),
          join(fixture.home, ".codex", "agents", `maestro-${renderedName}.toml`),
        ]) {
          expect({ toml, text: await readFile(toml, "utf8") }).toEqual({ toml, text: expect.not.stringContaining("AskUserQuestion") });
        }
      }
    }
  });
}, 90_000);

// seat-config-dirs R1 (d848, d849): the two new keys parse, settings is a
// Claude seat key only, and an unknown skill is refused naming the profile.
test("seat-dirs-keys: parseProfile accepts skills and settings, refuses settings on a codex or non-seat profile, planProfileRenders refuses an unknown skill naming the profile (R1)", async () => {
  const seat = parseProfile(
    "/profiles/lead.md",
    "---\nharness: claude\nmodel: default\nskills: [maestro-work, maestro-design]\nsettings:\n  outputStyle: Concise\n  autoMemoryEnabled: null\n---\nRole: Lead.\n",
  );
  expect(seat.frontmatter.skills).toEqual(["maestro-work", "maestro-design"]);
  expect(seat.frontmatter.settings).toEqual({ outputStyle: "Concise", autoMemoryEnabled: null });
  const node = parseProfile("/profiles/refuter.md", "---\nharness: claude\nmodel: default\nskills: [maestro-explore]\n---\nRole: Refuter.\n");
  expect(node.frontmatter.skills).toEqual(["maestro-explore"]);
  expect(node.frontmatter.settings).toBeUndefined();

  for (const [path, frontmatter] of [
    ["/profiles/lead.md", "harness: codex\nmodel: default\nsettings: {outputStyle: Concise}\n"],
    ["/profiles/refuter.md", "harness: claude\nmodel: default\nsettings: {outputStyle: Concise}\n"],
    ["/profiles/peer-opus.md", "harness: claude\nmodel: opus\nsettings: {outputStyle: Concise}\n"],
    ["/profiles/lead.md", "harness: claude\nmodel: default\nsettings: [a]\n"],
    ["/profiles/lead.md", "harness: claude\nmodel: default\nskills: maestro-work\n"],
  ] as const) {
    let message = "";
    try {
      parseProfile(path, `---\n${frontmatter}---\nRole: x.\n`);
    } catch (error) {
      message = error instanceof Error ? error.message : String(error);
    }
    expect({ path, frontmatter, message }).toEqual({
      path,
      frontmatter,
      message: expect.stringContaining(path),
    });
    expect(message).toContain(frontmatter.includes("skills:") ? "skills" : "settings");
  }

  await withFixture(async (fixture) => {
    const repoProfiles = join(fixture.repo, ".maestro", "profiles");
    const written = await writeProfile(
      repoProfiles,
      "lead",
      "---\nharness: claude\nmodel: default\nskills: [no-such-skill]\n---\nRole: Lead.\n",
    );
    let message = "";
    try {
      await planProfileRenders(fixture.home, fixture.repo);
    } catch (error) {
      message = error instanceof Error ? error.message : String(error);
    }
    expect(message).toContain(written);
    expect(message).toContain("no-such-skill");
    expect(message).toContain(join(fixture.home, "maestro", "skills"));
    expect(message).toContain(join(fixture.home, ".claude", "skills"));
  });
});

// seat-config-dirs R2 (d845, A5): seats and every peer-* render live in the
// seat dirs, bare nodes stay in ~/.claude/agents, the old seat files there are
// removed on materialize, and uninstall removes the whole seat root.
test("seat-dirs-targets: planProfileRenders puts lead, peer, team-supervisor and peer-* under <home>/.maestro/claude/<seat>/agents and bare nodes under <home>/.claude/agents; materializeProfiles removes a stale <home>/.claude/agents/maestro-lead.md; removeRenderedProfiles removes <home>/.maestro/claude (R2)", async () => {
  await withFixture(async (fixture) => {
    const claudeAgents = join(fixture.home, ".claude", "agents");
    const seatRoot = join(fixture.home, ".maestro", "claude");
    const claudePaths = (await planProfileRenders(fixture.home, fixture.repo))
      .map((target) => target.path)
      .filter((path) => path.endsWith(".md"));
    const byName = new Map(claudePaths.map((path) => [/^maestro-(.+)\.md$/.exec(basename(path))?.[1] ?? path, path]));
    expect(byName.get("lead")).toBe(join(seatRoot, "lead", "agents", "maestro-lead.md"));
    expect(byName.get("peer")).toBe(join(seatRoot, "peer", "agents", "maestro-peer.md"));
    expect(byName.get("team-supervisor")).toBe(join(seatRoot, "team-supervisor", "agents", "maestro-team-supervisor.md"));
    expect(byName.get("peer-opus")).toBe(join(seatRoot, "peer", "agents", "maestro-peer-opus.md"));
    expect(byName.get("peer-refuter")).toBe(join(seatRoot, "peer", "agents", "maestro-peer-refuter.md"));
    expect(byName.get("peer-reviewer-security")).toBe(join(seatRoot, "peer", "agents", "maestro-peer-reviewer-security.md"));
    for (const node of ["refuter", "reviewer-security", "auditor", "classifier", "synthesizer"]) {
      expect({ node, path: byName.get(node) }).toEqual({ node, path: join(claudeAgents, `maestro-${node}.md`) });
    }
    for (const [name, path] of byName) {
      const inSeatDir = path.startsWith(seatRoot);
      const isSeat = ["lead", "peer", "team-supervisor"].includes(name) || name.startsWith("peer-");
      expect({ name, inSeatDir }).toEqual({ name, inSeatDir: isSeat });
    }
    expect(renderedProfilePath(fixture.home, "claude", "lead")).toBe(byName.get("lead")!);
    expect(renderedProfilePath(fixture.home, "claude", "peer-refuter")).toBe(byName.get("peer-refuter")!);
    expect(renderedProfilePath(fixture.home, "claude", "refuter")).toBe(byName.get("refuter")!);

    await mkdir(claudeAgents, { recursive: true });
    const stale = join(claudeAgents, "maestro-lead.md");
    const stalePeer = join(claudeAgents, "maestro-peer-refuter.md");
    const custom = join(claudeAgents, "custom.md");
    for (const path of [stale, stalePeer]) await writeFile(path, "---\nname: old\n---\n\nold render\n");
    await writeFile(custom, "---\nname: custom\n---\nhand written\n");
    const sync = await materializeProfiles(fixture.home, fixture.repo);
    expect(sync.removed.sort()).toEqual([stale, stalePeer].sort());
    expect(existsSync(stale)).toBe(false);
    expect(existsSync(stalePeer)).toBe(false);
    expect(await readFile(custom, "utf8")).toBe("---\nname: custom\n---\nhand written\n");
    expect(existsSync(join(seatRoot, "lead", "agents", "maestro-lead.md"))).toBe(true);
    expect(existsSync(join(claudeAgents, "maestro-refuter.md"))).toBe(true);
    const staleNames = (await readdir(claudeAgents)).filter((entry) => /^maestro-(lead|peer|team-supervisor|peer-.+)\.md$/.test(entry));
    expect(staleNames).toEqual([]);

    const removed = await removeRenderedProfiles(fixture.home);
    expect(removed).toContain(seatRoot);
    expect(existsSync(seatRoot)).toBe(false);
    expect(existsSync(join(claudeAgents, "maestro-refuter.md"))).toBe(false);
    expect(await readFile(custom, "utf8")).toBe("---\nname: custom\n---\nhand written\n");
  });
});
