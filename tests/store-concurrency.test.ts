import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  addLinkedWorktree,
  idFrom,
  initializeGitRepository,
  runCli,
  runCliAt,
  withFixture,
} from "./helpers.ts";

function sessionEnvironment(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string, pane: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    `concurrent lane ${pane}`,
    "--owned-scope",
    "scratch fixture",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "contract stored",
    "--lane",
    "decision",
    "--evidence-required",
    "source: concurrent CLI readback",
    "--pane",
    pane,
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(x\d+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

function handbackArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "concurrent handback persisted",
    "--proof",
    "source: concurrent CLI readback",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

function handbackId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(h\d+) \[DONE\]/);
  if (!match?.[1]) throw new Error(`missing handback id in stdout: ${result.stdout}`);
  return match[1];
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
});

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
      await writeFile(join(plugins, "start-delay.ts"), delayPlugin);
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
});

test("291 concurrent decision, dispatch, and handback allocation stays serialized", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "entity-id-worktree");
    await addLinkedWorktree(fixture.repo, worktree);
    const work = idFrom(
      await runCli(fixture, ["work", "add", "concurrent entities", "--atomic-reason", "fixture"]),
    );

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

    // The council is declared before the race so the two concurrent opens join
    // one recorded generation instead of racing to infer one (d706).
    const anchor = await runCli(
      fixture,
      [...dispatchOpenArgs(work, "race:anchor"), "--council-members", "3"],
      sessionEnvironment("dispatch-open-a"),
    );
    expect(anchor.exitCode).toBe(0);
    const councilAnchor = dispatchId(anchor);
    const dispatches = await Promise.all([
      runCli(
        fixture,
        [...dispatchOpenArgs(work, "race:a"), "--council-anchor", councilAnchor],
        sessionEnvironment("dispatch-open-a"),
      ),
      runCliAt(
        fixture,
        worktree,
        [...dispatchOpenArgs(work, "race:b"), "--council-anchor", councilAnchor],
        sessionEnvironment("dispatch-open-b"),
      ),
    ]);
    expect(dispatches.map((result) => result.exitCode)).toEqual([0, 0]);
    expect(dispatches.map((result) => result.stderr).join("\n")).not.toMatch(
      /INTERNAL|database is locked|UNIQUE constraint/,
    );
    const dispatchIds = dispatches.map(dispatchId);
    expect(new Set(dispatchIds).size).toBe(2);
    const council = await runCli(fixture, ["dispatch", "list", work]);
    expect(council.stdout).toContain("council: sealed (0/3 returned)");

    const holders = ["handback-a", "handback-b"];
    const openers = ["dispatch-open-a", "dispatch-open-b"];
    for (const [index, dispatch] of dispatchIds.entries()) {
      expect(
        (
          await runCli(
            fixture,
            ["dispatch", "accept", dispatch],
            sessionEnvironment(holders[index] as string),
          )
        ).exitCode,
      ).toBe(0);
      expect(
        (
          await runCli(
            fixture,
            ["dispatch", "confirm", dispatch, "--session", holders[index] as string],
            sessionEnvironment(openers[index] as string),
          )
        ).exitCode,
      ).toBe(0);
    }
    const handbacks = await Promise.all([
      runCli(
        fixture,
        handbackArgs(dispatchIds[0] as string),
        sessionEnvironment(holders[0] as string),
      ),
      runCliAt(
        fixture,
        worktree,
        handbackArgs(dispatchIds[1] as string),
        sessionEnvironment(holders[1] as string),
      ),
    ]);
    expect(handbacks.map((result) => result.exitCode)).toEqual([0, 0]);
    expect(new Set(handbacks.map(handbackId)).size).toBe(2);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const eventCounts = database
      .query<{ count: number; type: string }, []>(
        `SELECT type, COUNT(*) AS count
         FROM event_log
         WHERE type IN ('decision.draft', 'dispatch.open', 'handback.file')
         GROUP BY type
         ORDER BY type`,
      )
      .all();
    database.close();
    expect(eventCounts).toEqual([
      { count: 2, type: "decision.draft" },
      { count: 3, type: "dispatch.open" },
      { count: 2, type: "handback.file" },
    ]);
  });
});

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
    await writeFile(join(fixture.repo, ".maestro", "plugins", "transition-delay.ts"), delayPlugin);

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
});

test("293 dispatch cancel and handback file have one terminal winner", async () => {
  await withFixture(async (fixture) => {
    for (let trial = 0; trial < 12; trial += 1) {
      const work = idFrom(
        await runCli(fixture, ["work", "add", `dispatch terminal race ${trial}`, "--atomic-reason", "fixture"]),
      );
      const holder = `terminal-holder-${trial}`;
      const dispatch = dispatchId(
        await runCli(fixture, dispatchOpenArgs(work, `terminal:${trial}`)),
      );
      expect(
        (
          await runCli(
            fixture,
            ["dispatch", "accept", dispatch],
            sessionEnvironment(holder),
          )
        ).exitCode,
      ).toBe(0);
      expect(
        (
          await runCli(fixture, [
            "dispatch",
            "confirm",
            dispatch,
            "--session",
            holder,
          ])
        ).exitCode,
      ).toBe(0);
      const outcomes = await Promise.all([
        runCli(
          fixture,
          ["dispatch", "cancel", dispatch, "--reason", "terminal race"],
          sessionEnvironment("lead"),
        ),
        runCli(fixture, handbackArgs(dispatch), sessionEnvironment(holder)),
      ]);
      expect(outcomes.filter((result) => result.exitCode === 0)).toHaveLength(1);
      const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
        readonly: true,
      });
      const row = database
        .query<{ cancelled: number; returned: number }, [string]>(
          `SELECT cancelled_at IS NOT NULL AS cancelled,
                  EXISTS(SELECT 1 FROM handbacks WHERE dispatch_id = dispatches.id) AS returned
           FROM dispatches WHERE id = ?`,
        )
        .get(dispatch);
      database.close();
      expect(Number(row?.cancelled) + Number(row?.returned)).toBe(1);
    }
  });
});

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
});

test("307 concurrent startup adds one pane column to an old dispatch schema", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const worktree = join(fixture.root, "dispatch-migration-worktree");
    await addLinkedWorktree(fixture.repo, worktree);
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const legacy = new Database(databasePath);
    legacy.exec("ALTER TABLE dispatches DROP COLUMN pane");
    legacy.close();

    const startups = await Promise.all([
      runCli(fixture, ["version"], sessionEnvironment("migration-a")),
      runCliAt(fixture, worktree, ["version"], sessionEnvironment("migration-b")),
    ]);
    expect(startups.map((result) => result.exitCode)).toEqual([0, 0]);

    const migrated = new Database(databasePath, { readonly: true });
    expect(
      migrated
        .query<{ name: string }, []>("PRAGMA table_info(dispatches)")
        .all()
        .filter((column) => column.name === "pane"),
    ).toHaveLength(1);
    migrated.close();
  });
});
