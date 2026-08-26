import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, type Fixture, withFixture } from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(pid) };
}

function probeDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

async function stalledLease(fixture: Fixture): Promise<void> {
  const parent = idFrom(
    await runCli(fixture, ["work", "add", "parent", "--atomic-reason", "probe fixture"]),
  );
  expect((await runCli(fixture, ["work", "start", parent], session("lead"))).exitCode).toBe(0);
  const child = idFrom(
    await runCli(fixture, ["work", "add", "child", "--parent", parent, "--kind", "task"]),
  );
  expect((await runCli(fixture, ["work", "start", child], session("subject", 1))).exitCode).toBe(0);
  const database = probeDatabase(fixture);
  try {
    database
      .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 45 * 60_000).toISOString(), "subject");
  } finally {
    database.close();
  }
}

test("220 no session may be recorded under the reserved system author id", async () => {
  await withFixture(async (fixture) => {
    const squatted = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("supervisor"),
    );
    expect(squatted.exitCode).not.toBe(0);
    expect(squatted.stderr).toContain("RESERVED_SESSION");

    const database = probeDatabase(fixture);
    try {
      expect(
        database
          .query<{ count: number }, [string]>("SELECT count(*) AS count FROM sessions WHERE id = ?")
          .get("supervisor")?.count,
      ).toBe(0);
    } finally {
      database.close();
    }
  });
});

test("221 mail addressed to the supervisor says it receives none", async () => {
  await withFixture(async (fixture) => {
    const sent = await runCli(fixture, ["msg", "send", "supervisor", "who are you?"], session("lead"));
    expect(sent.exitCode).not.toBe(0);
    expect(sent.stderr).toContain("SYSTEM_AUTHOR");
    expect(sent.stderr).toContain("does not receive mail");
    expect(sent.stderr).not.toContain("maestro status");
  });
});

test("222 the inbox marks a packet the supervisor authored", async () => {
  await withFixture(async (fixture) => {
    await stalledLease(fixture);
    expect((await runCli(fixture, ["attention"], session("scanner"))).exitCode).toBe(0);
    const database = probeDatabase(fixture);
    try {
      database.query("UPDATE messages SET sender_session = 'supervisor'").run();
    } finally {
      database.close();
    }

    const inbox = await runCli(fixture, ["msg", "read"], session("lead"));
    expect(inbox.exitCode).toBe(0);
    expect(inbox.stdout).toContain("from supervisor (system)");
  });
});
