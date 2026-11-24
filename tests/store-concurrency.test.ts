import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { grantTrust } from "../src/plugins/plugin-trust.ts";
import {
  addLinkedWorktree,
  idFrom,
  initializeGitRepository,
  runCli,
  runCliAt,
  withFixture,
} from "./helpers.ts";

// Every test here stages a real race by spawning many CLI processes, so wall
// time tracks machine load rather than the code under test. Bun's 5s default
// made them pass on an idle laptop and fail on a busy one; 293 failed a release
// verification that way. The explicit timeout is not slack for slow code, it is
// the acknowledgement that these measure concurrency, not speed.
function sessionEnvironment(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

test("B3.10 concurrent shared-store startup and work creation avoid locks and duplicate IDs", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "concurrent-worktree");
    await addLinkedWorktree(fixture.repo, worktree);

    const statuses = await Promise.all(
      Array.from({ length: 12 }, (_, index) =>
        runCliAt(
          fixture,
          index % 2 === 0 ? fixture.repo : worktree,
          ["status"],
          sessionEnvironment(`status-${index}`),
        ),
      ),
    );
    expect(
      statuses
        .filter((result) => result.exitCode !== 0)
        .map((result) => result.stderr.trim()),
    ).toEqual([]);

    const additions = await Promise.all(
      Array.from({ length: 20 }, (_, index) =>
        runCliAt(
          fixture,
          index % 2 === 0 ? fixture.repo : worktree,
          ["work", "add", `concurrent item ${index}`],
          sessionEnvironment(`add-${index}`),
        ),
      ),
    );
    expect(
      additions
        .filter((result) => result.exitCode !== 0)
        .map((result) => result.stderr.trim()),
    ).toEqual([]);
    const ids = additions.map(idFrom);
    expect(new Set(ids).size).toBe(additions.length);
  });
}, 120_000);

test("290 concurrent starts preserve the single lease winner and its audit event", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "lease-worktree");
    await addLinkedWorktree(fixture.repo, worktree);
    const parent = idFrom(await runCli(fixture, ["work", "add", "lease parent"]));
    const delayPlugin = `
export default {
  name: "start-delay",
  apply(context) {
    context.effect(() => context.events.on("work.start", async (input, next) => {
      await Bun.sleep(300);
      return next(input);
    }));
  },
};
`;
    for (const checkout of [fixture.repo, worktree]) {
      const plugins = join(checkout, ".maestro", "plugins");
      await mkdir(plugins, { recursive: true });
      const path = join(plugins, "start-delay.ts");
      await writeFile(path, delayPlugin);
      // The plugin is the instrument this test measures with; without a grant
      // it never loads and the race it stages never happens.
      await grantTrust(fixture.home, { root: path, source: "repo" });
    }

    for (let trial = 0; trial < 6; trial += 1) {
      const target = idFrom(
        await runCli(fixture, ["work", "add", `lease target ${trial}`, "--parent", parent]),
      );
      const contenders = [`lease-${trial}-a`, `lease-${trial}-b`];
      const results = await Promise.all([
        runCli(fixture, ["work", "start", target], sessionEnvironment(contenders[0] as string)),
        runCliAt(
          fixture,
          worktree,
          ["work", "start", target],
          sessionEnvironment(contenders[1] as string),
        ),
      ]);
      const successes = results.filter((result) => result.exitCode === 0);
      const failures = results.filter((result) => result.exitCode !== 0);
      expect(successes).toHaveLength(1);
      expect(failures).toHaveLength(1);
      const winner = successes[0]?.stdout.match(/started by (\S+)/)?.[1];
      expect(winner).toBeOneOf(contenders);
      if (!winner) throw new Error("missing lease winner in success output");
      expect(failures[0]?.stderr).toContain('"code":"LEASE_HELD"');
      const failureLine = failures[0]?.stderr
        .split("\n")
        .find((line) => line.includes('"code":"LEASE_HELD"')) ?? "{}";
      const failureEnvelope = JSON.parse(failureLine) as {
        error?: { holder?: string };
      };
      expect(failureEnvelope.error?.holder).toBe(winner);

      const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
        readonly: true,
      });
      const persisted = database
        .query<{ held_by: string | null; state: string }, [string]>(
          "SELECT state, held_by FROM work WHERE id = ?",
        )
        .get(target);
      const events = database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM event_log WHERE type = 'work.start' AND entity_id = ?",
        )
        .get(target)?.count;
      database.close();
      expect(persisted).toEqual({ held_by: winner, state: "active" });
      expect(events).toBe(1);
    }
  });
}, 120_000);

test("291 concurrent decision allocation stays serialized", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "entity-id-worktree");
    await addLinkedWorktree(fixture.repo, worktree);

    const decisions = await Promise.all([
      runCli(fixture, ["decision", "draft", "concurrent decision A"], sessionEnvironment("decision-a")),
      runCliAt(
        fixture,
        worktree,
        ["decision", "draft", "concurrent decision B"],
        sessionEnvironment("decision-b"),
      ),
    ]);
    expect(decisions.map((result) => result.exitCode)).toEqual([0, 0]);
    expect(decisions.map((result) => result.stderr).join("\n")).not.toMatch(
      /INTERNAL|database is locked|UNIQUE constraint/,
    );
    expect(new Set(decisions.map(idFrom)).size).toBe(2);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const eventCounts = database
      .query<{ count: number; type: string }, []>(
        `SELECT type, COUNT(*) AS count
         FROM event_log
         WHERE type IN ('decision.draft')
         GROUP BY type
         ORDER BY type`,
      )
      .all();
    database.close();
    expect(eventCounts).toEqual([
      { count: 2, type: "decision.draft" },
    ]);
  });
}, 120_000);

test("292 delayed work transitions cannot commit after their authorizing state changes", async () => {
  await withFixture(async (fixture) => {
    const delayPlugin = `
export default {
  name: "transition-delay",
  apply(context) {
    for (const event of ["work.start", "work.done", "work.cancel"]) {
      context.effect(() => context.events.on(event, async (input, next) => {
        if (process.env.MAESTRO_TEST_DELAY_EVENT === event) await Bun.sleep(300);
        return next(input);
      }));
    }
  },
};
`;
    const delayPath = join(fixture.repo, ".maestro", "plugins", "transition-delay.ts");
    await writeFile(delayPath, delayPlugin);
    await grantTrust(fixture.home, { root: delayPath, source: "repo" });

    const startThenCancel = idFrom(
      await runCli(fixture, ["work", "add", "start then cancel", "--atomic-reason", "fixture"]),
    );
    const delayedStart = runCli(
      fixture,
      ["work", "start", startThenCancel],
      { ...sessionEnvironment("start-lane"), MAESTRO_TEST_DELAY_EVENT: "work.start" },
    );
    await Bun.sleep(75);
    const cancellation = await runCli(
      fixture,
      ["work", "cancel", startThenCancel, "--reason", "cancelled during start gate"],
      sessionEnvironment("cancel-lane"),
    );
    const started = await delayedStart;
    expect(cancellation.exitCode).toBe(0);
    expect(started.exitCode).not.toBe(0);
    const cancelled = JSON.parse(
      (await runCli(fixture, ["work", "show", startThenCancel, "--json"])).stdout,
    ) as { data: { work: { heldBy: string | null; state: string } } };
    expect(cancelled.data.work).toEqual(expect.objectContaining({ heldBy: null, state: "cancelled" }));

    const cancelThenStart = idFrom(
      await runCli(fixture, ["work", "add", "cancel then start", "--atomic-reason", "fixture"]),
    );
    const delayedCancel = runCli(
      fixture,
      ["work", "cancel", cancelThenStart, "--reason", "cancelled during start"],
      { ...sessionEnvironment("cancel-lane"), MAESTRO_TEST_DELAY_EVENT: "work.cancel" },
    );
    await Bun.sleep(75);
    const claimed = await runCli(
      fixture,
      ["work", "start", cancelThenStart],
      sessionEnvironment("new-holder"),
    );
    const cancelledAfterClaim = await delayedCancel;
    expect(claimed.exitCode).toBe(0);
    expect(cancelledAfterClaim.exitCode).not.toBe(0);
    const active = JSON.parse(
      (await runCli(fixture, ["work", "show", cancelThenStart, "--json"])).stdout,
    ) as { data: { work: { heldBy: string | null; state: string } } };
    expect(active.data.work).toEqual(
      expect.objectContaining({ heldBy: "new-holder", state: "active" }),
    );

    const doneThenReclaim = idFrom(
      await runCli(fixture, ["work", "add", "done then reclaim", "--atomic-reason", "fixture"]),
    );
    expect(
      (
        await runCli(
          fixture,
          ["work", "start", doneThenReclaim],
          sessionEnvironment("done-holder"),
        )
      ).exitCode,
    ).toBe(0);
    const delayedDone = runCli(
      fixture,
      [
        "work",
        "done",
        doneThenReclaim,
        "--claim",
        "transition finished",
        "--proof",
        "source: focused race",
      ],
      { ...sessionEnvironment("done-holder"), MAESTRO_TEST_DELAY_EVENT: "work.done" },
    );
    await Bun.sleep(75);
    const reclaimed = await runCli(
      fixture,
      ["work", "reclaim", doneThenReclaim, "--reason", "owner transferred"],
      sessionEnvironment("replacement-holder"),
    );
    const completed = await delayedDone;
    expect(reclaimed.exitCode).toBe(0);
    expect(completed.exitCode).not.toBe(0);
    const afterReclaim = JSON.parse(
      (await runCli(fixture, ["work", "show", doneThenReclaim, "--json"])).stdout,
    ) as { data: { work: { heldBy: string | null; state: string } } };
    expect(afterReclaim.data.work).toEqual(
      expect.objectContaining({ heldBy: "replacement-holder", state: "active" }),
    );
  });
}, 120_000);

test("306 concurrent attention scans raise one row and one event", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "attention-worktree");
    await addLinkedWorktree(fixture.repo, worktree);
    const work = idFrom(await runCli(fixture, ["work", "add", "attention race"]));
    const decision = idFrom(
      await runCli(fixture, ["decision", "draft", "stale attention race", "--work", work]),
    );
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const writable = new Database(databasePath);
    writable
      .query("UPDATE decisions SET created_at = ? WHERE id = ?")
      .run(new Date(Date.now() - 25 * 60 * 60_000).toISOString(), decision);
    writable.close();

    const scans = await Promise.all([
      runCli(
        fixture,
        ["attention", "--json", "--decision-stale", "24"],
        sessionEnvironment("attention-a"),
      ),
      runCliAt(
        fixture,
        worktree,
        ["attention", "--json", "--decision-stale", "24"],
        sessionEnvironment("attention-b"),
      ),
    ]);
    expect(scans.map((result) => result.exitCode)).toEqual([0, 0]);
    const raised = scans.map((result) => {
      const envelope = JSON.parse(result.stdout) as {
        data: { detections: Array<{ fingerprint: string; raised: boolean }> };
      };
      return envelope.data.detections.find(
        (detection) => detection.fingerprint === `decision:${decision}`,
      )?.raised;
    });
    expect(raised.sort()).toEqual([false, true]);

    const stored = new Database(databasePath, { readonly: true });
    expect(
      stored
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM attention WHERE fingerprint = ?",
        )
        .get(`decision:${decision}`)?.count,
    ).toBe(1);
    expect(
      stored
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM event_log WHERE type = 'attention.raise' AND entity_id = ?",
        )
        .get(decision)?.count,
    ).toBe(1);
    stored.close();
  });
}, 120_000);
