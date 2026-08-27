import { Database } from "bun:sqlite";
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

function backdateSession(fixture: Fixture, id: string, minutes: number): void {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  try {
    database
      .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - minutes * 60_000).toISOString(), id);
  } finally {
    database.close();
  }
}

test("160 work start declares an existing item atomic instead of forcing a duplicate", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "rate limiter: add the IP tier");

    const blocked = await runCli(fixture, ["work", "start", work], session("lead"));
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("GATE_BLOCKED");

    const started = await runCli(
      fixture,
      ["work", "start", work, "--atomic-reason", "single file, acceptance in one sentence"],
      session("lead"),
    );
    expect(started.exitCode).toBe(0);

    const shown = await runCli(fixture, ["work", "show", work]);
    expect(shown.stdout).toContain("[active]");
    expect(shown.stdout).toContain("atomic reason: single file, acceptance in one sentence");

    const listed = await runCli(fixture, ["work", "list"]);
    expect(listed.stdout).not.toContain("[cancelled]");
  });
});

test("161 the breakdown gate names the self-unblocking command for the blocked item", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "rate limiter: add the IP tier");
    const blocked = await runCli(fixture, ["work", "start", work], session("lead"));
    expect(blocked.stderr).toContain(`maestro work start ${work} --atomic-reason`);
  });
});

test("162 an atomic reason on start never bypasses the open-children gate", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope");
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child scope", "--parent", parent, "--kind", "task"]),
    );

    const blocked = await runCli(
      fixture,
      ["work", "start", parent, "--atomic-reason", "pretending this is atomic"],
      session("lead"),
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain(child);

    const shown = await runCli(fixture, ["work", "show", parent]);
    expect(shown.stdout).toContain("[open]");
  });
});

test("163 [lint] a stalled-lease packet renders the durable work command", async () => {
  await withFixture(async (fixture) => {
    // Proves human rendering text, not structured subject identity or execution of the action.
    const parent = await addWork(fixture, "parent scope", ["--atomic-reason", "fixture"]);
    expect((await runCli(fixture, ["work", "start", parent], session("lead-session"))).exitCode)
      .toBe(0);
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child scope", "--parent", parent, "--kind", "task"]),
    );
    expect((await runCli(fixture, ["work", "start", child], session("subject-session", 1))).exitCode)
      .toBe(0);
    backdateSession(fixture, "subject-session", 45);

    const attention = await runCli(fixture, ["attention"], session("scanner"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain(`attention STALLED_LEASE work ${child}`);
    expect(attention.stdout).toContain(`smallest action: maestro work show ${child}`);
  });
});

test("164 a scope collision is recorded once after the overlap banner", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope", ["--atomic-reason", "fixture"]);
    const first = idFrom(
      await runCli(fixture, ["work", "add", "first lane", "--parent", parent, "--kind", "task"]),
    );
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second lane", "--parent", parent, "--kind", "task"]),
    );
    // The later starter is the one the work-start overlap banner warns; the
    // earlier holder never learns that a sibling lane opened.
    expect((await runCli(fixture, ["work", "start", first], session("early-holder"))).exitCode)
      .toBe(0);
    const later = await runCli(fixture, ["work", "start", second], session("late-holder"));
    expect(later.exitCode).toBe(0);
    expect(later.stderr).toContain("[overlap]");

    const attention = await runCli(fixture, ["attention"], session("scanner"));
    expect(attention.stdout).toContain(`attention SCOPE_COLLISION work ${first},${second}`);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      expect(
        database.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count,
      ).toBe(1);
    } finally {
      database.close();
    }
  });
});

test("165 doctor reports Codex hook trust as unverified without a documented hash contract", async () => {
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
    const stillUnverified = await runCli(fixture, ["doctor"], { PATH: path });
    expect(stillUnverified.exitCode).toBe(0);
    expect(stillUnverified.stdout).toContain("codex hooks: unverified");
    expect(stillUnverified.stdout).toContain("/hooks");
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
    const loop = skill.slice(skill.indexOf("## Loop"), skill.indexOf("## Test-first"));
    // The installed prose assertion is lint; the attention result proves the detector contract.
    expect(loop).toContain('maestro work note <id> "failed: ');

    const work = await addWork(fixture, "repeat the failing implementation", [
      "--atomic-reason",
      "fixture",
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
