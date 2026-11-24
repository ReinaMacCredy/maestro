import { expect, test } from "bun:test";
import { join } from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import {
  idFrom,
  initializeGitRepository,
  prepareInstallFixture,
  runCli,
  runTool,
  type Fixture,
  withFixture,
} from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

async function addWork(fixture: Fixture, title: string, extra: string[] = []): Promise<string> {
  return idFrom(await runCli(fixture, ["work", "add", title, ...extra]));
}

test("162 work start on a parent with an open child is refused by the open-children gate", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope");
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child scope", "--parent", parent, "--kind", "task"]),
    );

    const blocked = await runCli(
      fixture,
      ["work", "start", parent],
      session("lead"),
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain(child);

    const shown = await runCli(fixture, ["work", "show", parent]);
    expect(shown.stdout).toContain("[open]");
  });
});

test("165 doctor reports Codex hook trust as unverified until Codex records both events, never as verified", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const untrusted = await runCli(fixture, ["doctor"], { PATH: path });
    expect(untrusted.exitCode).toBe(0);
    expect(untrusted.stdout).toContain("codex hooks: unverified");
    expect(untrusted.stdout).toContain("/hooks");

    await mkdir(join(fixture.home, ".codex"), { recursive: true });
    await writeFile(
      join(fixture.home, ".codex", "config.toml"),
      `[hooks.state."${join(fixture.repo, ".codex", "hooks.json")}:session_start:0:0"]\n` +
        `trusted_hash = "sha256:deadbeef"\n` +
        `[hooks.state."${join(fixture.repo, ".codex", "hooks.json")}:user_prompt_submit:0:0"]\n` +
        `trusted_hash = "sha256:deadbeef"\n`,
    );
    const recorded = await runCli(fixture, ["doctor"], { PATH: path });
    expect(recorded.exitCode).toBe(0);
    expect(recorded.stdout).toContain("codex hooks: recorded by Codex");
    expect(recorded.stdout).toContain("hash not verifiable");
    expect(recorded.stdout).not.toContain("codex hooks: trusted");
  });
});

test("166 bundle open stamps the base commit git already knows", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const head = (await runTool(["git", "rev-parse", "--short", "HEAD"], fixture.repo)).stdout.trim();
    const branch = (await runTool(["git", "branch", "--show-current"], fixture.repo)).stdout.trim();

    expect((await runCli(fixture, ["bundle", "open", "stamped"])).exitCode).toBe(0);
    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "stamped", "NOTES.md"),
    ).text();
    expect(notes).toContain(`Base: ${head} (${branch})`);
  });

  await withFixture(async (fixture) => {
    // A directory that is not a git checkout still scaffolds, with Base left blank.
    expect((await runCli(fixture, ["bundle", "open", "unstamped"])).exitCode).toBe(0);
    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "unstamped", "NOTES.md"),
    ).text();
    expect(notes).toContain("Base:\n");
  });
});

test("167 documented failed-pass notes trigger repeated-failure attention", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const skill = await Bun.file(
      join(fixture.home, "maestro", "skills", "maestro-work", "SKILL.md"),
    ).text();
    const loop = skill.slice(skill.indexOf("## Record"), skill.indexOf("## Writing rules"));
    // The installed prose assertion is lint; the attention result proves the detector contract.
    expect(loop).toContain('maestro work note <id> "failed: ');

    const work = await addWork(fixture, "repeat the failing implementation", [
    ]);
    for (const note of [
      "failed: first mechanism",
      "failed: second mechanism",
      "failed: third mechanism",
    ]) {
      expect((await runCli(fixture, ["work", "note", work, note])).exitCode).toBe(0);
    }
    const attention = await runCli(fixture, ["attention", "--json"]);
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: { detections: Array<{ kind: string; subjectWork: string | null }> };
    }).data.detections;
    expect(detections).toContainEqual(expect.objectContaining({
      kind: "REPEATED_FAILURE",
      subjectWork: work,
    }));
  });
});
