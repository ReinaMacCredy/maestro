import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture } from "./helpers.ts";

test("7 policy-proof blocks a claim without matching proof", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "inspect", "--kind", "idea"]);
    const id = idFrom(added);
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);

    const result = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "checks pass",
      "--evidence",
      "raw evidence without a proof",
    ]);

    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("policy-proof");
  });
});

test("8 disabling policy-proof removes its flags while core evidence still completes verbatim", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["plugin", "disable", "policy-proof"])).exitCode).toBe(0);
    const added = await runCli(fixture, ["work", "add", "inspect", "--kind", "idea"]);
    const id = idFrom(added);
    expect((await runCli(fixture, ["work", "start", id])).exitCode).toBe(0);
    const evidence = "raw: checks=missing; keep  spacing & punctuation";

    const rejected = await runCli(fixture, [
      "work",
      "done",
      id,
      "--claim",
      "checks pass",
      "--evidence",
      evidence,
    ]);
    const completed = await runCli(fixture, [
      "work",
      "done",
      id,
      "--evidence",
      evidence,
    ]);
    const shown = await runCli(fixture, ["work", "show", id]);

    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("unknown flag");
    expect(rejected.stderr).toContain("--claim");
    expect(completed.exitCode).toBe(0);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(evidence);
  });
});

test("9 policy-breakdown blocks childless write-like work without an atomic reason", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "implement", "--kind", "feature"]);

    const result = await runCli(fixture, ["work", "start", idFrom(added)]);

    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("policy-breakdown");
  });
});

test("10 ready excludes blocked work and promotes it after its blocker completes", async () => {
  await withFixture(async (fixture) => {
    const first = idFrom(await runCli(fixture, ["work", "add", "first", "--kind", "idea"]));
    const second = idFrom(
      await runCli(fixture, ["work", "add", "second", "--kind", "idea", "--blocked-by", first]),
    );

    const before = await runCli(fixture, ["ready"]);
    expect(before.stdout).toContain(first);
    expect(before.stdout).not.toContain(second);

    expect((await runCli(fixture, ["work", "start", first])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", first, "--evidence", "finished"])).exitCode).toBe(0);
    const after = await runCli(fixture, ["ready"]);

    expect(after.exitCode).toBe(0);
    expect(after.stdout).toContain(second);
  });
});

test("11 live leases refuse a second session and dead-session leases expire passively", async () => {
  await withFixture(async (fixture) => {
    const holder = Bun.spawn(["sleep", "30"]);
    try {
      const id = idFrom(await runCli(fixture, ["work", "add", "shared", "--kind", "idea"]));
      const claimed = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-a",
        MAESTRO_SESSION_PID: String(holder.pid),
      });
      const refused = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });

      expect(claimed.exitCode).toBe(0);
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("session-a");

      holder.kill();
      await holder.exited;
      const reclaimed = await runCli(fixture, ["work", "start", id], {
        MAESTRO_SESSION_ID: "session-b",
        MAESTRO_SESSION_PID: String(process.pid),
      });
      expect(reclaimed.exitCode).toBe(0);
    } finally {
      holder.kill();
    }
  });
});

test("12 write verbs append events and the store rejects mutation of prior log rows", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "logged", "--kind", "idea"]);
    expect(added.exitCode).toBe(0);

    const { Store } = await import("../src/kernel/store.ts");
    const store = new Store(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      const count = store.database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM event_log")
        .get()?.count;
      expect(count).toBeGreaterThan(0);
      expect(() => store.database.run("UPDATE event_log SET type = 'changed'")).toThrow();
      expect(() => store.database.run("DELETE FROM event_log")).toThrow();
    } finally {
      store.close();
    }
  });
});
