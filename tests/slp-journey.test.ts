import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, type Fixture, withFixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
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

test("163 a stalled-lease packet asks the silent holder instead of only reading the row", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "parent scope", ["--atomic-reason", "fixture"]);
    expect((await runCli(fixture, ["work", "start", parent], session("lead-session"))).exitCode)
      .toBe(0);
    const child = idFrom(
      await runCli(fixture, ["work", "add", "child scope", "--parent", parent, "--kind", "task"]),
    );
    expect((await runCli(fixture, ["work", "start", child], session("subject-session"))).exitCode)
      .toBe(0);
    backdateSession(fixture, "subject-session", 45);

    const attention = await runCli(fixture, ["attention"], session("scanner"));
    expect(attention.exitCode).toBe(0);
    expect(attention.stdout).toContain("attention STALLED_LEASE");
    expect(attention.stdout).toContain("smallest action: maestro msg send subject-session");
  });
});

test("164 a scope collision tells the holder who never saw the overlap banner", async () => {
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
    expect(attention.stdout).toContain("attention SCOPE_COLLISION");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      const targets = database
        .query<{ target_session: string }, []>("SELECT target_session FROM messages")
        .all()
        .map((row) => row.target_session);
      expect(targets).toEqual(["early-holder"]);
      expect(
        database.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count,
      ).toBe(1);
    } finally {
      database.close();
    }
  });
});
