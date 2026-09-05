import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
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
  const text = await readFile(join(fixture.home, ".claude", "agents", `maestro-${name}.md`), "utf8");
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
        const claude = await readFile(join(fixture.home, ".claude", "agents", `maestro-${renderedName}.md`), "utf8");
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
