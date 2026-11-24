import { expect, test } from "bun:test";
import { mkdir, readdir, readFile, rm, stat, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  materializeSeats,
  parseSeatFile,
  seatConfigRoot,
  seatDirectory,
  seatLaunchPath,
  seatTokenPath,
  type SeatName,
} from "../src/plugins/slp-seats.ts";
import { materializeSkills } from "../src/plugins/skills.ts";
import { prepareInstallFixture, runCli, withFixture, type Fixture } from "./helpers.ts";

async function writeSeatFile(directory: string, seat: SeatName, text: string): Promise<string> {
  await mkdir(directory, { recursive: true });
  const path = join(directory, `${seat}.md`);
  await writeFile(path, text);
  return path;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

async function claudeBody(fixture: Fixture, seat: SeatName): Promise<string> {
  const text = await readFile(seatLaunchPath(fixture.home, "claude", seat), "utf8");
  return text.slice(text.indexOf("\n---\n") + "\n---\n".length);
}

async function messageOf(action: () => unknown): Promise<string> {
  try {
    await action();
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "";
}

test("seat-file-parse: every key renders, each malformed seat file fails install naming the file and key, and a repo seat file shadows the owner's, which shadows the shipped one (red 1)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    const repoSeats = join(fixture.repo, ".maestro", "profiles");
    const ownerSeats = join(fixture.home, "maestro", "profiles");
    await writeSeatFile(
      ownerSeats,
      "lead",
      "---\nharness: claude\nmodel: opus\neffort: high\npermission: acceptEdits\nautocompact: 250000\ndisallowed_tools: [Write, Edit]\ndescription: every key set\n---\nRole: every key.\n\nThe body is the mandate.\n",
    );
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const rendered = await readFile(seatLaunchPath(fixture.home, "claude", "lead"), "utf8");
    for (const line of ["name: maestro-lead\n", "model: opus\n", "effort: high\n", "permissionMode: acceptEdits\n", "disallowedTools: Write, Edit\n"]) {
      expect(rendered).toContain(line);
    }
    expect((await claudeBody(fixture, "lead")).endsWith("\nRole: every key.\n\nThe body is the mandate.\n")).toBe(true);

    for (const [frontmatter, key] of [
      ["harness: claude\nmodel: default\ncolour: red\n", "colour"],
      ["harness: gemini\nmodel: default\n", "harness"],
      ["harness: codex\nmodel: default\neffort: extreme\n", "effort"],
    ] as const) {
      const written = await writeSeatFile(ownerSeats, "peer", `---\n${frontmatter}---\nRole: peer.\n`);
      const refused = await runCli(fixture, ["install"], { PATH: path });
      expect(refused.exitCode).toBe(1);
      expect(failure(refused.stderr).code).toBe("INVALID_SEAT_FILE");
      expect(failure(refused.stderr).message).toContain(written);
      expect(failure(refused.stderr).message).toContain(key);
    }
    await rm(join(ownerSeats, "peer.md"));

    await writeSeatFile(repoSeats, "lead", "---\nharness: claude\nmodel: default\n---\nRole: repo lead.\n");
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    expect(await claudeBody(fixture, "lead")).toContain("Role: repo lead.");
    expect(await claudeBody(fixture, "lead")).not.toContain("every key");
  });
}, 90_000);

test("package-json: no dependencies key (red 10, A5)", async () => {
  const packageJson = JSON.parse(
    await readFile(join(import.meta.dir, "..", "package.json"), "utf8"),
  ) as Record<string, unknown>;
  expect("dependencies" in packageJson).toBe(false);
});

// seat-config-dirs R1 (d848, d849): skills and settings parse, a malformed one
// is refused naming the file, and an unknown skill is refused naming the file.
test("seat-dirs-keys: parseSeatFile accepts skills and settings, refuses a settings list or a skills string, and materializeSeats refuses an unknown skill naming the seat file (R1)", async () => {
  const seat = parseSeatFile(
    "/owner/lead.md",
    "---\nharness: claude\nmodel: default\nskills: [maestro-work, maestro-design]\nsettings:\n  outputStyle: Concise\n  autoMemoryEnabled: null\n---\nRole: Lead.\n",
    "owner",
  );
  expect(seat.config.skills).toEqual(["maestro-work", "maestro-design"]);
  expect(seat.config.settings).toEqual({ outputStyle: "Concise", autoMemoryEnabled: null });
  expect(await messageOf(() => parseSeatFile("/owner/peer.md", "---\nharness: codex\nsandbox: read-only\n---\n", "owner"))).toBe("");
  for (const [frontmatter, key] of [
    ["harness: claude\nmodel: default\nsettings: [a]\n", "settings"],
    ["harness: claude\nmodel: default\nskills: maestro-work\n", "skills"],
  ] as const) {
    const message = await messageOf(() => parseSeatFile("/owner/lead.md", `---\n${frontmatter}---\nRole: x.\n`, "owner"));
    expect(message).toContain("/owner/lead.md");
    expect(message).toContain(key);
  }

  await withFixture(async (fixture) => {
    const written = await writeSeatFile(
      join(fixture.home, "maestro", "profiles"),
      "lead",
      "---\nharness: claude\nmodel: default\nskills: [no-such-skill]\n---\nRole: Lead.\n",
    );
    const message = await messageOf(() => materializeSeats(fixture.home, fixture.repo));
    expect(message).toContain(written);
    expect(message).toContain("no-such-skill");
    expect(message).toContain(join(fixture.home, "maestro", "skills"));
    expect(message).toContain(join(fixture.home, ".claude", "skills"));
  });
});

// seat-config-dirs R3 (d847, d849, A4): the seat settings are the maestro base
// merged with the seat file overlay, null removes, 0600, the token rides env,
// the herdr SessionStart hook is present and cleanupPeriodDays never is.
test("seat-dirs-settings: seat settings.json is base merged with the overlay (null removes) at 0600 with the token in env and the herdr hook, no cleanupPeriodDays; the Claude seat file carries the d847 disallowedTools (R3)", async () => {
  await withFixture(async (fixture) => {
    const seatRoot = seatConfigRoot(fixture.home);
    await mkdir(seatRoot, { recursive: true });
    await writeFile(seatTokenPath(fixture.home), "sk-ant-oat01-fixture-token\n");
    await materializeSkills(fixture.home, "dev");
    await writeSeatFile(
      join(fixture.home, "maestro", "profiles"),
      "lead",
      "---\nharness: claude\nmodel: opus\nskills: [maestro-work]\ndisallowed_tools: [Agent, LSP, \"Bash(claude:*)\"]\nsettings:\n  outputStyle: Concise\n  autoMemoryEnabled: null\n  env:\n    BASH_DEFAULT_TIMEOUT_MS: \"300000\"\n    CLAUDE_CODE_DISABLE_FAST_MODE: null\n---\nRole: fixture lead.\n",
    );
    await materializeSeats(fixture.home, fixture.repo);

    const hook = {
      matcher: "*",
      hooks: [{ type: "command", command: `bash '${join(fixture.home, ".claude", "hooks", "herdr-agent-state.sh")}' session`, timeout: 10 }],
    };
    const leadSettings = join(seatDirectory(fixture.home, "lead"), "settings.json");
    expect(JSON.parse(await readFile(leadSettings, "utf8"))).toEqual({
      permissions: { defaultMode: "bypassPermissions", deny: ["Bash(claude:*)", "Bash(npx claude:*)"] },
      skipDangerousModePermissionPrompt: true,
      hooks: { SessionStart: [hook] },
      env: {
        CLAUDE_CODE_OAUTH_TOKEN: "sk-ant-oat01-fixture-token",
        CLAUDE_CODE_DISABLE_WORKFLOWS: "1",
        CLAUDE_CODE_DISABLE_CRON: "1",
        CLAUDE_CODE_DISABLE_BUNDLED_SKILLS: "1",
        CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS: "1",
        BASH_DEFAULT_TIMEOUT_MS: "300000",
      },
      disableWorkflows: true,
      workflowKeywordTriggerEnabled: false,
      attribution: { commit: "", pr: "", sessionUrl: false },
      enabledPlugins: {},
      claudeMdExcludes: [join(fixture.home, ".claude", "CLAUDE.md"), join(fixture.home, ".claude", "rules", "**")],
      outputStyle: "Concise",
    });
    expect((await stat(leadSettings)).mode & 0o777).toBe(0o600);
    expect((await stat(seatDirectory(fixture.home, "lead"))).mode & 0o777).toBe(0o700);
    expect((await stat(seatRoot)).mode & 0o777).toBe(0o700);

    const peerText = await readFile(join(seatDirectory(fixture.home, "peer"), "settings.json"), "utf8");
    const peer = JSON.parse(peerText) as Record<string, unknown>;
    expect(peerText).not.toContain("cleanupPeriodDays");
    expect(peerText).not.toContain("language");
    expect(peer.hooks).toEqual({ SessionStart: [hook] });
    expect((peer.env as Record<string, string>).CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat01-fixture-token");
    expect((peer.env as Record<string, string>).CLAUDE_CODE_DISABLE_FAST_MODE).toBe("1");
    expect(peer.autoMemoryEnabled).toBe(false);
    expect((await stat(join(seatDirectory(fixture.home, "peer"), "settings.json"))).mode & 0o777).toBe(0o600);
    for (const tool of ["Bash(claude:*)", "Bash(npx claude:*)", "Bash(herdr:*)"]) {
      expect((peer.permissions as { deny: string[] }).deny).toContain(tool);
    }

    const denyLine = async (seat: SeatName) => {
      const text = await readFile(seatLaunchPath(fixture.home, "claude", seat), "utf8");
      return text.slice(0, text.indexOf("\n---\n", 4)).split("\n").find((line) => line.startsWith("disallowedTools: ")) ?? "";
    };
    for (const tool of ["SlashCommand", "WebSearch", "EnterPlanMode", "ExitPlanMode", "AskUserQuestion"]) {
      expect({ tool, line: await denyLine("peer") }).toEqual({ tool, line: expect.stringContaining(tool) });
    }
    expect(await denyLine("lead")).toBe("disallowedTools: Agent, LSP");
  });
});

// seat-config-dirs R9 (d852): the owner's ~/.claude/CLAUDE.md matches the
// project-scope pattern for <home>, so the base excludes it by absolute path.
test("seat-dirs-claudemd-excludes: every seat settings.json carries claudeMdExcludes naming the owner's <home>/.claude/CLAUDE.md and <home>/.claude/rules/** as absolute paths (R9, d852)", async () => {
  await withFixture(async (fixture) => {
    await materializeSkills(fixture.home, "dev");
    await materializeSeats(fixture.home, fixture.repo);
    for (const seat of ["lead", "peer"] as const) {
      const text = await readFile(join(seatDirectory(fixture.home, seat), "settings.json"), "utf8");
      const parsed = JSON.parse(text) as Record<string, unknown>;
      expect({ seat, claudeMdExcludes: parsed.claudeMdExcludes }).toEqual({
        seat,
        claudeMdExcludes: [join(fixture.home, ".claude", "CLAUDE.md"), join(fixture.home, ".claude", "rules", "**")],
      });
      expect({ seat, text }).toEqual({ seat, text: expect.not.stringContaining("~") });
    }
  });
});

// seat-config-dirs R11 (d854): a Bash(...) pattern on the agent disallowedTools
// line drops the whole Bash tool, so patterns go to the seat deny.
test("seat-dirs-deny-split: the lead agent disallowedTools line carries no ( entry; the lead settings deny is exactly the two Bash patterns; the peer deny carries Bash(herdr:*); an overlay permissions.deny is unioned with the split (R11, d854)", async () => {
  await withFixture(async (fixture) => {
    await materializeSkills(fixture.home, "dev");
    await writeSeatFile(
      join(fixture.home, "maestro", "profiles"),
      "peer",
      "---\nharness: claude\nmodel: opus\nskills: [maestro-work]\ndisallowed_tools: [Agent, \"Bash(claude:*)\"]\nsettings:\n  permissions:\n    deny: [\"WebFetch\"]\n---\nRole: fixture peer.\n",
    );
    await materializeSeats(fixture.home, fixture.repo);

    const denyOf = async (seat: SeatName) =>
      (JSON.parse(await readFile(join(seatDirectory(fixture.home, seat), "settings.json"), "utf8")) as { permissions: { deny?: string[] } }).permissions.deny;
    const leadText = await readFile(seatLaunchPath(fixture.home, "claude", "lead"), "utf8");
    const leadLine = leadText.slice(0, leadText.indexOf("\n---\n", 4)).split("\n").find((line) => line.startsWith("disallowedTools: ")) ?? "";
    expect(leadLine).not.toContain("(");
    expect(leadLine).not.toContain("Agent");
    expect(leadLine).toContain("LSP");
    expect(await denyOf("lead")).toEqual(["Bash(claude:*)", "Bash(npx claude:*)"]);
    expect(await denyOf("peer")).toEqual(["Bash(claude:*)", "Bash(npx claude:*)", "Bash(herdr:*)", "WebFetch"]);
    const peerText = await readFile(seatLaunchPath(fixture.home, "claude", "peer"), "utf8");
    expect(peerText.slice(0, peerText.indexOf("\n---\n", 4)).split("\n")).toContain("disallowedTools: Agent");
  });
});

// seat-config-dirs R12 (d847): the deny list is exact per seat, pinned as the
// whole string and the whole array, from the shipped seat files alone.
test("seat-dirs-exact-deny: the shipped lead and peer carry exactly the d847 disallowedTools line and settings permissions.deny array (R12, d847)", async () => {
  await withFixture(async (fixture) => {
    await materializeSkills(fixture.home, "dev");
    await materializeSeats(fixture.home, fixture.repo);

    const lineOf = async (seat: SeatName) => {
      const text = await readFile(seatLaunchPath(fixture.home, "claude", seat), "utf8");
      return text.slice(0, text.indexOf("\n---\n", 4)).split("\n").find((line) => line.startsWith("disallowedTools: ")) ?? "";
    };
    const denyOf = async (seat: SeatName) =>
      (JSON.parse(await readFile(join(seatDirectory(fixture.home, seat), "settings.json"), "utf8")) as { permissions: { deny?: string[] } }).permissions.deny;
    const five = "SlashCommand, WebSearch, EnterPlanMode, ExitPlanMode, AskUserQuestion";
    const twoBash = ["Bash(claude:*)", "Bash(npx claude:*)"];

    expect(await lineOf("lead")).toBe(`disallowedTools: ${five}, LSP`);
    expect(await denyOf("lead")).toEqual(twoBash);
    expect(await lineOf("peer")).toBe(`disallowedTools: ${five}`);
    expect(await denyOf("peer")).toEqual([...twoBash, "Bash(herdr:*)"]);
  });
});

// seat-config-dirs R13 (d855 rules 1 and 2): settings reaches env and hooks in
// a file that carries the seat token, skills reaches the filesystem, so a
// repository seat file cannot carry either; a skill name is a plain directory
// name and its target stays under the two skill roots.
test("seat-dirs-owner-only-keys: a repo seat file with settings: or skills: is refused naming the file and the owner dir while an owner copy renders; a skill name ../../victim, a/b or .. is refused; a skill whose realpath escapes the skill roots is refused (R13, d855)", async () => {
  const repoLead = "/repo/.maestro/profiles/lead.md";
  for (const [key, frontmatter] of [
    ["settings", "settings: {outputStyle: Concise}\n"],
    ["skills", "skills: [maestro-work]\n"],
  ] as const) {
    const message = await messageOf(() =>
      parseSeatFile(repoLead, `---\nharness: claude\nmodel: default\n${frontmatter}---\nRole: repo lead.\n`, "repo")
    );
    for (const part of [repoLead, key, "~/maestro/profiles", "shipped"]) {
      expect({ key, message }).toEqual({ key, message: expect.stringContaining(part) });
    }
  }
  expect(await messageOf(() => parseSeatFile(repoLead, "---\nharness: claude\nmodel: opus\n---\nRole: repo lead.\n", "repo"))).toBe("");
  const ownerLead = parseSeatFile(
    "/home/maestro/profiles/lead.md",
    "---\nharness: claude\nmodel: default\nskills: [maestro-work]\nsettings: {outputStyle: Concise}\n---\nRole: home lead.\n",
    "owner",
  );
  expect(ownerLead.config.skills).toEqual(["maestro-work"]);
  expect(ownerLead.config.settings).toEqual({ outputStyle: "Concise" });

  for (const name of ["../../victim", "a/b", "..", "Victim", ".hidden", ""]) {
    const message = await messageOf(() =>
      parseSeatFile("/home/maestro/profiles/peer.md", `---\nharness: claude\nmodel: default\nskills: [${JSON.stringify(name)}]\n---\nRole: peer.\n`, "owner")
    );
    expect({ name, message }).toEqual({ name, message: expect.stringContaining("/home/maestro/profiles/peer.md") });
    expect({ name, message }).toEqual({ name, message: expect.stringContaining("skills") });
  }
  expect(await messageOf(() =>
    parseSeatFile("/home/maestro/profiles/peer.md", "---\nharness: claude\nmodel: default\nskills: [maestro-work, my.skill_2-x]\n---\nRole: peer.\n", "owner")
  )).toBe("");

  await withFixture(async (fixture) => {
    await materializeSkills(fixture.home, "dev");
    const overlay = "settings:\n  outputStyle: Concise\n";
    const written = await writeSeatFile(join(fixture.repo, ".maestro", "profiles"), "lead", `---\nharness: claude\nmodel: default\n${overlay}---\nRole: repo lead.\n`);
    const refused = await messageOf(() => materializeSeats(fixture.home, fixture.repo));
    expect(refused).toContain(written);
    expect(refused).toContain("settings");
    expect(refused).toContain("~/maestro/profiles");
    await rm(written, { force: true });

    const ownerSeats = join(fixture.home, "maestro", "profiles");
    await writeSeatFile(ownerSeats, "lead", `---\nharness: claude\nmodel: default\nskills: [maestro-work]\n${overlay}---\nRole: home lead.\n`);
    await materializeSeats(fixture.home, fixture.repo);
    const leadSettings = JSON.parse(await readFile(join(seatDirectory(fixture.home, "lead"), "settings.json"), "utf8")) as Record<string, unknown>;
    expect(leadSettings.outputStyle).toBe("Concise");
    expect(await readdir(join(seatDirectory(fixture.home, "lead"), "skills"))).toEqual(["maestro-work"]);

    const victim = join(fixture.root, "victim");
    await mkdir(victim, { recursive: true });
    await writeFile(join(victim, "SKILL.md"), "---\nname: victim\n---\nvictim\n");
    await symlink(victim, join(fixture.home, "maestro", "skills", "escaped"));
    const escaping = await writeSeatFile(ownerSeats, "lead", "---\nharness: claude\nmodel: default\nskills: [escaped]\n---\nRole: home lead.\n");
    const message = await messageOf(() => materializeSeats(fixture.home, fixture.repo));
    expect(message).toContain(escaping);
    expect(message).toContain("escaped");
    expect(message).toContain(join(fixture.home, "maestro", "skills"));
  });
});

// seat-config-dirs R14 (d855 rule 3): the shipped seat's patterns seed every
// seat deny and an owner seat file can only add to them.
test("seat-dirs-deny-floor: an owner lead file that drops every Bash(...) entry still gets the shipped two patterns, an owner peer file keeps Bash(herdr:*), and a pattern it adds widens the list (R14, d855)", async () => {
  await withFixture(async (fixture) => {
    await materializeSkills(fixture.home, "dev");
    const ownerSeats = join(fixture.home, "maestro", "profiles");
    await writeSeatFile(ownerSeats, "lead", "---\nharness: claude\nmodel: opus\ndisallowed_tools: [Agent, LSP]\n---\nRole: narrowed lead.\n");
    await writeSeatFile(ownerSeats, "peer", "---\nharness: claude\nmodel: opus\ndisallowed_tools: [Agent, \"Bash(git push:*)\"]\n---\nRole: narrowed and widened peer.\n");
    await materializeSeats(fixture.home, fixture.repo);

    const denyOf = async (seat: SeatName) =>
      (JSON.parse(await readFile(join(seatDirectory(fixture.home, seat), "settings.json"), "utf8")) as { permissions: { deny?: string[] } }).permissions.deny;
    const lineOf = async (seat: SeatName) => {
      const text = await readFile(seatLaunchPath(fixture.home, "claude", seat), "utf8");
      return text.slice(0, text.indexOf("\n---\n", 4)).split("\n").find((line) => line.startsWith("disallowedTools: ")) ?? "";
    };
    const twoBash = ["Bash(claude:*)", "Bash(npx claude:*)"];
    expect(await denyOf("lead")).toEqual(twoBash);
    expect(await lineOf("lead")).toBe("disallowedTools: Agent, LSP");
    expect(await denyOf("peer")).toEqual([...twoBash, "Bash(herdr:*)", "Bash(git push:*)"]);
    expect(await lineOf("peer")).toBe("disallowedTools: Agent");
  });
});
