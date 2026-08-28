import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import {
  idFrom,
  initializeGitRepository,
  runCli,
  withFixture,
  type Fixture,
} from "./helpers.ts";

function session(id: string): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(process.pid),
  };
}

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Settle the storage boundary",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the contract is stored",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
    "--pane",
    "w1:pA",
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

function insertStoredDispatch(
  fixture: Fixture,
  input: { createdAt: string; id: string; lane: string; work: string },
): void {
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
  database
    .query(
      `INSERT INTO dispatches
        (id, work_id, objective, owned_scope, excluded_scope, mutation, stop_condition,
         lane, evidence_required, pane, opened_by, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      input.id,
      input.work,
      "historical record",
      "fixture",
      "product source",
      "no-write",
      "record remains readable",
      input.lane,
      "source: readback",
      "w1:pZ",
      "test-session",
      input.createdAt,
      input.createdAt,
    );
  database.close();
}

async function openDispatch(fixture: Fixture): Promise<string> {
  const work = idFrom(
    await runCli(fixture, ["work", "add", "handback contract", "--atomic-reason", "fixture"]),
  );
  const opened = await runCli(fixture, dispatchOpenArgs(work));
  expect(opened.exitCode).toBe(0);
  return dispatchId(opened);
}

async function acceptDispatch(
  fixture: Fixture,
  dispatch: string,
  holder = "test-session",
): Promise<void> {
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode)
    .toBe(0);
  await confirmDispatch(fixture, dispatch, holder);
}

async function confirmDispatch(
  fixture: Fixture,
  dispatch: string,
  holder: string,
): Promise<void> {
  expect(
    (
      await runCli(fixture, ["dispatch", "confirm", dispatch, "--session", holder])
    ).exitCode,
  ).toBe(0);
}

function handbackFileArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the contract is stored",
    "--proof",
    "source: focused test passes",
    "--assumptions",
    "None",
    "--residual-risks",
    "None",
    "--incidental-findings",
    "None",
  ];
}

function handbackId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[[A-Z_]+\]/);
  if (!match?.[1]) throw new Error(`missing handback id in stdout: ${result.stdout}`);
  return match[1];
}

async function openCouncil(
  fixture: Fixture,
): Promise<{ dispatches: [string, string]; work: string }> {
  const work = idFrom(
    await runCli(fixture, ["work", "add", "sealed council", "--atomic-reason", "fixture"]),
  );
  const first = await runCli(fixture, dispatchOpenArgs(work));
  const second = await runCli(fixture, dispatchOpenArgs(work));
  expect(first.exitCode).toBe(0);
  expect(second.exitCode).toBe(0);
  const dispatches: [string, string] = [dispatchId(first), dispatchId(second)];
  for (const dispatch of dispatches) await acceptDispatch(fixture, dispatch);
  return { dispatches, work };
}

test("173 dispatch open refuses every missing or blank envelope field", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "dispatch contract", "--atomic-reason", "fixture"]),
    );
    const fields = [
      "--objective",
      "--owned-scope",
      "--excluded-scope",
      "--mutation",
      "--stop-condition",
      "--lane",
      "--evidence-required",
      "--pane",
    ];

    for (const field of fields) {
      const args = dispatchOpenArgs(work);
      const index = args.indexOf(field);
      args.splice(index, 2);
      const missing = await runCli(fixture, args);
      expect(missing.exitCode).not.toBe(0);
      expect(missing.stderr).toContain(field);

      const blankArgs = dispatchOpenArgs(work);
      blankArgs[blankArgs.indexOf(field) + 1] = "   ";
      const blank = await runCli(fixture, blankArgs);
      expect(blank.exitCode).not.toBe(0);
      expect(blank.stderr).toContain(field);
    }
  });
});

test("300 dispatch open rejects blank target sessions without writing a contract or event", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "target session validation", "--atomic-reason", "fixture"]),
    );
    for (const target of ["", "   "]) {
      const attempted = await runCli(fixture, [
        ...dispatchOpenArgs(work),
        "--target-session",
        target,
      ]);
      expect(attempted.exitCode).not.toBe(0);
      expect(attempted.stderr).toContain('"code":"MISSING_ARGUMENT"');
      expect(attempted.stderr).toContain("missing or blank --target-session");
    }
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM dispatches").get()?.count,
    ).toBe(0);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM event_log WHERE type = 'dispatch.open'",
        )
        .get()?.count,
    ).toBe(0);
    database.close();
  });
});

test("174 [lint] dispatch show and list render the complete stored contract and identities", async () => {
  // Presentation lint: proves stored fields are rendered, not that their persisted values are correct.
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "render dispatch", "--atomic-reason", "fixture"]),
    );
    const opened = await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--target-session", "lane-one"],
    );
    expect(opened.exitCode).toBe(0);
    const id = dispatchId(opened);

    for (const command of [["dispatch", "show", id], ["dispatch", "list"]]) {
      const rendered = await runCli(fixture, command);
      expect(rendered.exitCode).toBe(0);
      for (const line of [
        `work: ${work}`,
        "objective: Settle the storage boundary",
        "owned scope: src/plugins/dispatch.ts",
        "excluded scope: push, tag, publish",
        "mutation: write-bounded: src/plugins/dispatch.ts",
        "stop condition: the contract is stored",
        "lane: delivery",
        "evidence required: source and live",
        "target session: lane-one",
        "opened by: test-session",
        "claimed by: none",
        "held by: none",
      ]) {
        expect(rendered.stdout).toContain(line);
      }
    }
  });
});

test("175 accepting dispatches never changes the work write lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "three lanes", "--atomic-reason", "fixture"]),
    );
    const lead = session("lead");
    expect((await runCli(fixture, ["work", "start", work], lead)).exitCode).toBe(0);
    const holders = ["lane-one", "lane-two", "lane-three"];
    const dispatches: string[] = [];
    for (const holder of holders) {
      const opened = await runCli(
        fixture,
        [...dispatchOpenArgs(work), "--target-session", holder],
      );
      expect(opened.exitCode).toBe(0);
      const id = dispatchId(opened);
      dispatches.push(id);
      expect((await runCli(fixture, ["dispatch", "accept", id], session(holder))).exitCode).toBe(0);
      const shown = await runCli(fixture, ["work", "show", work, "--json"]);
      expect(shown.exitCode).toBe(0);
      const showEnvelope = JSON.parse(shown.stdout) as {
        data: { work: { heldBy: string | null } };
      };
      expect(showEnvelope.data.work.heldBy).toBe("lead");
    }

    const listed = await runCli(fixture, ["dispatch", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const listEnvelope = JSON.parse(listed.stdout) as {
      data: { dispatches: Array<{ heldBy: string | null; id: string }> };
    };
    expect(
      listEnvelope.data.dispatches
        .filter((dispatch) => dispatches.includes(dispatch.id))
        .map((dispatch) => dispatch.heldBy),
    ).toEqual(holders);
  });
});

test("271 targeted accept requires equality while an untargeted dispatch stays compatible", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "targeted lane", "--atomic-reason", "fixture"]),
    );
    const targeted = dispatchId(
      await runCli(fixture, [...dispatchOpenArgs(work), "--target-session", "intended-lane"]),
    );

    const refused = await runCli(
      fixture,
      ["dispatch", "accept", targeted],
      session("different-lane"),
    );
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("TARGET_MISMATCH");
    expect(refused.stderr).toContain("intended-lane");
    expect(refused.stderr).toContain("different-lane");
    expect(
      (await runCli(fixture, ["dispatch", "accept", targeted], session("intended-lane")))
        .exitCode,
    ).toBe(0);

    const untargeted = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    expect(
      (await runCli(fixture, ["dispatch", "accept", untargeted], session("any-lane"))).exitCode,
    ).toBe(0);
  });
});

test("423 an unconfirmed dispatch claim cannot file a handback or take its delivery work lease", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const claimant = session("claiming-lane");
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], claimant)).exitCode).toBe(0);

    const shown = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(shown.stdout).toContain("claimed by: claiming-lane");
    expect(shown.stdout).toContain("held by: none");

    const refused = await runCli(fixture, handbackFileArgs(dispatch), claimant);
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain('"code":"DISPATCH_UNCONFIRMED"');
    expect(refused.stderr).toContain(
      `maestro dispatch confirm ${dispatch} --session claiming-lane`,
    );
  });

  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "unconfirmed delivery", "--atomic-reason", "fixture"]),
    );
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    const claimant = session("claiming-lane");
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], claimant)).exitCode).toBe(0);

    const refused = await runCli(fixture, ["work", "start", work], claimant);
    expect(refused.exitCode).not.toBe(0);
    const error = JSON.parse(refused.stderr) as {
      error: { code: string; message: string; origin: string };
    };
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-dispatch");
    expect(error.error.message).toContain(
      `maestro dispatch confirm ${dispatch} --session claiming-lane`,
    );
  });
});

test("424 only the dispatch opener can confirm an untargeted claim", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const claimant = session("claiming-lane");
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], claimant)).exitCode).toBe(0);

    const refused = await runCli(
      fixture,
      ["dispatch", "confirm", dispatch, "--session", "claiming-lane"],
      session("different-opener"),
    );
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain('"code":"DISPATCH_CONFIRM_FORBIDDEN"');

    const mismatch = await runCli(fixture, [
      "dispatch",
      "confirm",
      dispatch,
      "--session",
      "different-claimant",
    ]);
    expect(mismatch.exitCode).not.toBe(0);
    expect(mismatch.stderr).toContain('"code":"CLAIM_MISMATCH"');

    const confirmed = await runCli(fixture, [
      "dispatch",
      "confirm",
      dispatch,
      "--session",
      "claiming-lane",
    ]);
    expect(confirmed.exitCode).toBe(0);
    expect(confirmed.stdout).toContain("opened by: test-session");
    expect(confirmed.stdout).toContain("claimed by: none");
    expect(confirmed.stdout).toContain("held by: claiming-lane");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const event = database
      .query<{ payload: string; session_id: string }, [string]>(
        "SELECT payload, session_id FROM event_log WHERE type = 'dispatch.confirm' AND entity_id = ?",
      )
      .get(dispatch);
    database.close();
    expect(event?.session_id).toBe("test-session");
    expect(JSON.parse(event?.payload ?? "null")).toEqual({ sessionId: "claiming-lane" });

    expect((await runCli(fixture, handbackFileArgs(dispatch), claimant)).exitCode).toBe(0);
  });
});

test("426 handback file and show round-trip an optional opaque candidate", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    await acceptDispatch(fixture, dispatch);
    const candidate = "artifact digest: sha256:abc123";
    const args = handbackFileArgs(dispatch);
    args.push("--candidate", candidate);

    const filed = await runCli(fixture, args);
    expect(filed.exitCode).toBe(0);
    expect(filed.stdout).toContain(`candidate: ${candidate}`);

    const shown = await runCli(fixture, ["handback", "show", handbackId(filed)]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`candidate: ${candidate}`);
  });
});

test("272 handback file refuses a session that is not the dispatch holder", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session("holding-lane"))).exitCode,
    ).toBe(0);
    await confirmDispatch(fixture, dispatch, "holding-lane");

    const refused = await runCli(
      fixture,
      handbackFileArgs(dispatch),
      session("different-lane"),
    );
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("DISPATCH_HELD");
    expect(refused.stderr).toContain("holding-lane");
    expect(refused.stderr).toContain("different-lane");
    expect(
      (await runCli(fixture, handbackFileArgs(dispatch), session("holding-lane"))).exitCode,
    ).toBe(0);
  });
});

test("273 handback file refuses a second return for the same dispatch", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const holder = session("returning-lane");
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], holder)).exitCode).toBe(0);
    await confirmDispatch(fixture, dispatch, "returning-lane");
    expect((await runCli(fixture, handbackFileArgs(dispatch), holder)).exitCode).toBe(0);

    const repeated = await runCli(fixture, handbackFileArgs(dispatch), holder);
    expect(repeated.exitCode).not.toBe(0);
    expect(repeated.stderr).toContain("HANDBACK_EXISTS");
    expect(repeated.stderr).toContain(dispatch);
    expect(repeated.stderr).toContain("second dispatch");
    expect(repeated.stderr).toMatch(/maestro work note w\d+ \\?"after h\d+:/);
  });
});

test("274 work-scoped dispatch lists render compact lane state without changing the archive", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "lane board", "--atomic-reason", "fixture"]),
    );
    const first = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    const secondArgs = dispatchOpenArgs(work);
    secondArgs[secondArgs.indexOf("--pane") + 1] = "w1:pB";
    const second = dispatchId(await runCli(fixture, secondArgs));
    const liveHolder = session("live-lane");
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "SessionStart"], liveHolder))
        .exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", first], liveHolder)).exitCode).toBe(0);
    await confirmDispatch(fixture, first, "live-lane");
    expect(
      (await runCli(fixture, ["dispatch", "accept", second], session("dead-lane"))).exitCode,
    ).toBe(0);
    await confirmDispatch(fixture, second, "dead-lane");

    const databaseBefore = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const archiveBefore = {
      dispatches: databaseBefore.query<Record<string, unknown>, []>(
        "SELECT * FROM dispatches ORDER BY id",
      ).all(),
      eventLogCount: databaseBefore.query<{ count: number }, []>(
        "SELECT COUNT(*) AS count FROM event_log",
      ).get()?.count,
    };
    databaseBefore.close();

    const scoped = await runCli(fixture, ["dispatch", "list", work]);
    expect(scoped.exitCode).toBe(0);
    expect(scoped.stdout).toContain("council: sealed (0/2 returned)");
    expect(scoped.stdout).toContain(
      `lane w1:pA | ${first} | delivery | dispatch=open | work=open | holder=live`,
    );
    expect(scoped.stdout).toContain(
      `lane w1:pB | ${second} | delivery | dispatch=open | work=open | holder=dead`,
    );
    expect(scoped.stdout).not.toContain("claim:");

    const databaseAfter = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const archiveAfter = {
      dispatches: databaseAfter.query<Record<string, unknown>, []>(
        "SELECT * FROM dispatches ORDER BY id",
      ).all(),
      eventLogCount: databaseAfter.query<{ count: number }, []>(
        "SELECT COUNT(*) AS count FROM event_log",
      ).get()?.count,
    };
    databaseAfter.close();
    expect(archiveAfter).toEqual(archiveBefore);

    const archive = await runCli(fixture, ["dispatch", "list"]);
    expect(archive.exitCode).toBe(0);
    expect(archive.stdout).toContain(`work: ${work}`);
    expect(archive.stdout).toContain("objective: Settle the storage boundary");
    expect(archive.stdout).toContain("held by: live-lane");

    const terminal = idFrom(
      await runCli(fixture, ["work", "add", "finished lane", "--atomic-reason", "fixture"]),
    );
    const lead = session("lead");
    expect((await runCli(fixture, ["work", "start", terminal], lead)).exitCode).toBe(0);
    const cancelled = dispatchId(await runCli(fixture, dispatchOpenArgs(terminal), lead));
    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "cancel", cancelled, "--reason", "lane completed elsewhere"],
          lead,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["work", "done", terminal, "--evidence", "source: fixture"],
          lead,
        )
      ).exitCode,
    ).toBe(0);

    const finished = await runCli(fixture, ["dispatch", "list", terminal]);
    expect(finished.exitCode).toBe(0);
    expect(finished.stdout).toBe(
      `lane w1:pA | ${cancelled} | delivery | dispatch=cancelled | work=done | holder=none\n`,
    );
    expect(finished.stdout).not.toContain("[open]");
  });
});

test("277 a delivery dispatch after a completed council does not reseal its handbacks", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const first = await runCli(fixture, handbackFileArgs(council.dispatches[0]));
    expect(first.exitCode).toBe(0);
    expect(
      (await runCli(fixture, handbackFileArgs(council.dispatches[1]))).exitCode,
    ).toBe(0);
    const firstHandback = handbackId(first);

    const completed = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(completed.exitCode).toBe(0);
    expect(completed.stdout).toContain("council: complete (2/2 returned)");

    const followUp = dispatchId(await runCli(fixture, dispatchOpenArgs(council.work)));
    const listed = await runCli(fixture, ["dispatch", "list", council.work]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("council: complete (2/2 returned)");
    expect(listed.stdout).toContain(
      `lane w1:pA | ${followUp} | delivery | dispatch=open | work=open | holder=none`,
    );
    expect(listed.stdout).not.toContain("council: sealed");

    const stillReadable = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(stillReadable.exitCode).toBe(0);
    expect(stillReadable.stdout).toContain("council: complete (2/2 returned)");
  });
});

test("278 two sequential dispatches on one work item never form a council", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "sequential lanes", "--atomic-reason", "fixture"]),
    );
    const first = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    await acceptDispatch(fixture, first);
    const firstReturn = await runCli(fixture, handbackFileArgs(first));
    expect(firstReturn.exitCode).toBe(0);

    const second = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    const whileOpen = await runCli(fixture, ["dispatch", "list", work]);
    expect(whileOpen.exitCode).toBe(0);
    expect(whileOpen.stdout).not.toContain("council:");
    expect(
      (await runCli(fixture, ["handback", "show", handbackId(firstReturn)])).exitCode,
    ).toBe(0);

    await acceptDispatch(fixture, second);
    expect((await runCli(fixture, handbackFileArgs(second))).exitCode).toBe(0);
    const completed = await runCli(fixture, ["dispatch", "list", work]);
    expect(completed.exitCode).toBe(0);
    expect(completed.stdout).not.toContain("council:");

    const cancelledWork = idFrom(
      await runCli(fixture, ["work", "add", "cancelled lane", "--atomic-reason", "fixture"]),
    );
    const cancelled = dispatchId(await runCli(fixture, dispatchOpenArgs(cancelledWork)));
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "cancel",
          cancelled,
          "--reason",
          "resolved without a return",
        ])
      ).exitCode,
    ).toBe(0);
    dispatchId(await runCli(fixture, dispatchOpenArgs(cancelledWork)));
    const afterCancellation = await runCli(fixture, ["dispatch", "list", cancelledWork]);
    expect(afterCancellation.exitCode).toBe(0);
    expect(afterCancellation.stdout).not.toContain("council:");
  });
});

test("279 a genuine later council seals only its own concurrent generation", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "later council", "--atomic-reason", "fixture"]),
    );
    const earlier = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    await acceptDispatch(fixture, earlier);
    const earlierReturn = await runCli(fixture, handbackFileArgs(earlier));
    expect(earlierReturn.exitCode).toBe(0);

    const createdAt = new Date().toISOString();
    insertStoredDispatch(fixture, { createdAt, id: "x2", lane: "design", work });
    insertStoredDispatch(fixture, { createdAt, id: "x3", lane: "design", work });
    const first = "x2";
    const second = "x3";
    await acceptDispatch(fixture, first);
    await acceptDispatch(fixture, second);
    const firstReturn = await runCli(fixture, handbackFileArgs(first));
    expect(firstReturn.exitCode).toBe(0);

    const priorGeneration = await runCli(fixture, [
      "handback",
      "show",
      handbackId(earlierReturn),
    ]);
    expect(priorGeneration.exitCode).toBe(0);
    expect(priorGeneration.stdout).not.toContain("council:");

    const sealed = await runCli(fixture, ["handback", "show", handbackId(firstReturn)]);
    expect(sealed.exitCode).not.toBe(0);
    expect(sealed.stderr).toContain("SEALED");
    expect(sealed.stderr).toContain("1/2 returned");
    const listed = await runCli(fixture, ["dispatch", "list", work]);
    expect(listed.stdout).toContain("council: sealed (1/2 returned)");

    expect((await runCli(fixture, handbackFileArgs(second))).exitCode).toBe(0);
    const opened = await runCli(fixture, ["handback", "show", handbackId(firstReturn)]);
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("council: complete (2/2 returned)");
  });
});

test("296 unsealing one council generation never opens a later generation", async () => {
  await withFixture(async (fixture) => {
    const firstGeneration = await openCouncil(fixture);
    const unsealed = await runCli(fixture, [
      "dispatch",
      "unseal",
      firstGeneration.work,
      "--reason",
      "the first generation can be reviewed early",
    ]);
    expect(unsealed.exitCode).toBe(0);
    expect(unsealed.stdout).toContain("council: unsealed (0/2 returned)");
    for (const dispatch of firstGeneration.dispatches) {
      expect((await runCli(fixture, handbackFileArgs(dispatch))).exitCode).toBe(0);
    }

    const secondGeneration = [
      dispatchId(await runCli(fixture, dispatchOpenArgs(firstGeneration.work))),
      dispatchId(await runCli(fixture, dispatchOpenArgs(firstGeneration.work))),
    ];
    for (const dispatch of secondGeneration) await acceptDispatch(fixture, dispatch);
    const partial = await runCli(fixture, handbackFileArgs(secondGeneration[0] as string));
    expect(partial.exitCode).toBe(0);
    const sealed = await runCli(fixture, ["handback", "show", handbackId(partial)]);
    expect(sealed.exitCode).not.toBe(0);
    expect(sealed.stderr).toContain("SEALED");
    const listed = await runCli(fixture, ["dispatch", "list", firstGeneration.work]);
    expect(listed.stdout).toContain("council: sealed (1/2 returned)");
    expect(listed.stdout).not.toContain("council: unsealed");

    expect((await runCli(fixture, handbackFileArgs(secondGeneration[1] as string))).exitCode)
      .toBe(0);
    const completedUnseal = await runCli(fixture, [
      "dispatch",
      "unseal",
      firstGeneration.work,
      "--reason",
      "completed generations cannot be unsealed",
    ]);
    expect(completedUnseal.exitCode).not.toBe(0);
    expect(completedUnseal.stderr).toContain("INVALID_STATE");
  });
});

test("297 legacy work-scoped council unseals migrate to the first generation anchor", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "legacy council unseal", "--atomic-reason", "fixture"]),
    );
    const first = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath);
    database.exec(`
      DROP TABLE dispatch_councils;
      CREATE TABLE dispatch_councils (
        work_id TEXT PRIMARY KEY REFERENCES work(id),
        unsealed_at TEXT NOT NULL,
        unseal_reason TEXT NOT NULL
      );
    `);
    database
      .query(
        "INSERT INTO dispatch_councils(work_id, unsealed_at, unseal_reason) VALUES (?, ?, ?)",
      )
      .run(work, new Date().toISOString(), "legacy operator choice");
    database.close();

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    database = new Database(databasePath, { readonly: true });
    const columns = database
      .query<{ name: string }, []>("PRAGMA table_info(dispatch_councils)")
      .all()
      .map((column) => column.name);
    const migrated = database
      .query<
        { generation_anchor: string; unseal_reason: string; work_id: string },
        []
      >("SELECT work_id, generation_anchor, unseal_reason FROM dispatch_councils")
      .get();
    database.close();
    expect(columns).toContain("generation_anchor");
    expect(migrated).toEqual({
      generation_anchor: first,
      unseal_reason: "legacy operator choice",
      work_id: work,
    });
    expect((await runCli(fixture, ["dispatch", "list", work])).stdout).toContain(
      "council: unsealed (0/2 returned)",
    );
  });
});

test("280 handoff exposes completed generations without leaking a sealed generation", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const work = idFrom(
      await runCli(fixture, ["work", "add", "generation handoff", "--atomic-reason", "fixture"]),
    );
    expect(
      (await runCli(fixture, ["bundle", "open", "generation-handoff", "--work", work])).exitCode,
    ).toBe(0);

    const earlier = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    await acceptDispatch(fixture, earlier);
    const earlierArgs = handbackFileArgs(earlier);
    earlierArgs[earlierArgs.indexOf("--claim") + 1] = "prior generation is complete";
    expect((await runCli(fixture, earlierArgs)).exitCode).toBe(0);

    const first = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    const second = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    await acceptDispatch(fixture, first);
    await acceptDispatch(fixture, second);
    const currentArgs = handbackFileArgs(first);
    currentArgs[currentArgs.indexOf("--claim") + 1] = "sealed generation partial return";
    expect((await runCli(fixture, currentArgs)).exitCode).toBe(0);

    const handedOff = await runCli(fixture, ["handoff", "generation-handoff", "--json"]);
    expect(handedOff.exitCode).toBe(0);
    const envelope = JSON.parse(handedOff.stdout) as {
      data: { handbacks: Array<{ dispatchId: string }> };
    };
    expect(envelope.data.handbacks.map((handback) => handback.dispatchId)).toEqual([earlier]);
    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "generation-handoff", "NOTES.md"),
    ).text();
    expect(notes).toContain("prior generation is complete");
    expect(notes).not.toContain("sealed generation partial return");
  });
});

test("281 dispatch lanes share the brief vocabulary and preserve historical unknown values", async () => {
  await withFixture(async (fixture) => {
    const lanes = ["scout", "decision", "delivery", "challenge", "shadow"] as const;
    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("lane-vocabulary"),
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      "lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write)",
    );

    const work = idFrom(
      await runCli(fixture, ["work", "add", "lane validation", "--atomic-reason", "fixture"]),
    );
    const invalidArgs = dispatchOpenArgs(work);
    invalidArgs[invalidArgs.indexOf("--lane") + 1] = "design";
    const invalid = await runCli(fixture, invalidArgs);
    expect(invalid.exitCode).not.toBe(0);
    expect(invalid.stderr).toContain("INVALID_LANE");
    expect(invalid.stderr).toContain(
      "expected one of: scout, decision, delivery, challenge, shadow",
    );

    for (const lane of lanes) {
      const args = dispatchOpenArgs(work);
      args[args.indexOf("--lane") + 1] = lane;
      const opened = await runCli(fixture, args);
      expect(opened.exitCode).toBe(0);
      expect(opened.stdout).toContain(`lane: ${lane}`);
    }
    const listed = JSON.parse(
      (await runCli(fixture, ["dispatch", "list", "--json"])).stdout,
    ) as { data: { dispatches: Array<{ lane: string; workId: string }> } };
    expect(
      listed.data.dispatches
        .filter((dispatch) => dispatch.workId === work)
        .map((dispatch) => dispatch.lane),
    ).toEqual([...lanes]);

    const historicalWork = idFrom(
      await runCli(fixture, ["work", "add", "historical lane", "--atomic-reason", "fixture"]),
    );
    const now = new Date().toISOString();
    insertStoredDispatch(fixture, {
      createdAt: now,
      id: "x900",
      lane: "design",
      work: historicalWork,
    });

    const historical = await runCli(fixture, ["dispatch", "show", "x900"]);
    expect(historical.exitCode).toBe(0);
    expect(historical.stdout).toContain("lane: design");
    const readback = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      readback.query<{ lane: string }, [string]>("SELECT lane FROM dispatches WHERE id = ?")
        .get("x900")?.lane,
    ).toBe("design");
    readback.close();
  });
});

test("263 returned and cancelled dispatches clear their lane holder", async () => {
  await withFixture(async (fixture) => {
    const returned = await openDispatch(fixture);
    expect(
      (await runCli(fixture, ["dispatch", "accept", returned], session("returning-lane")))
        .exitCode,
    ).toBe(0);
    await confirmDispatch(fixture, returned, "returning-lane");
    expect(
      (await runCli(fixture, handbackFileArgs(returned), session("returning-lane"))).exitCode,
    ).toBe(0);

    const cancelled = await openDispatch(fixture);
    expect(
      (await runCli(fixture, ["dispatch", "accept", cancelled], session("cancelled-lane")))
        .exitCode,
    ).toBe(0);
    await confirmDispatch(fixture, cancelled, "cancelled-lane");
    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "cancel",
          cancelled,
          "--reason",
          "lane was abandoned",
        ])
      ).exitCode,
    ).toBe(0);

    const listed = await runCli(fixture, ["dispatch", "list", "--json"]);
    expect(listed.exitCode).toBe(0);
    const envelope = JSON.parse(listed.stdout) as {
      data: { dispatches: Array<{ heldBy: string | null; id: string }> };
    };
    expect(
      envelope.data.dispatches
        .filter((dispatch) => [returned, cancelled].includes(dispatch.id))
        .map((dispatch) => dispatch.heldBy),
    ).toEqual([null, null]);
  });
});

test("266 dispatch migration clears legacy terminal holders once", async () => {
  await withFixture(async (fixture) => {
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const legacy = new Database(databasePath, { create: true });
    legacy.exec(`
      CREATE TABLE dispatches (
        id TEXT PRIMARY KEY,
        work_id TEXT NOT NULL REFERENCES work(id),
        objective TEXT NOT NULL,
        owned_scope TEXT NOT NULL,
        excluded_scope TEXT NOT NULL,
        mutation TEXT NOT NULL,
        stop_condition TEXT NOT NULL,
        lane TEXT NOT NULL,
        evidence_required TEXT NOT NULL,
        pane TEXT,
        target_session TEXT,
        held_by TEXT,
        cancelled_at TEXT,
        cancel_reason TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE handbacks (
        id TEXT PRIMARY KEY,
        dispatch_id TEXT NOT NULL REFERENCES dispatches(id),
        status TEXT NOT NULL,
        claim TEXT NOT NULL,
        proof TEXT NOT NULL,
        assumptions TEXT NOT NULL,
        residual_risks TEXT NOT NULL,
        incidental_findings TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      INSERT INTO dispatches VALUES
        ('legacy-returned', 'w1', 'o', 's', 'e', 'no-write', 'returned', 'delivery',
         'source', 'w1:pA', NULL, 'returned-lane', NULL, NULL,
         '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'),
        ('legacy-cancelled', 'w2', 'o', 's', 'e', 'no-write', 'cancelled', 'delivery',
         'source', 'w1:pB', NULL, 'cancelled-lane', '2026-01-02T00:00:00.000Z', 'abandoned',
         '2026-01-01T00:00:00.000Z', '2026-01-02T00:00:00.000Z'),
        ('legacy-open', 'w3', 'o', 's', 'e', 'no-write', 'open', 'delivery',
         'source', 'w1:pC', NULL, 'open-lane', NULL, NULL,
         '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
      INSERT INTO handbacks VALUES
        ('legacy-handback', 'legacy-returned', 'DONE', 'returned', 'source: fixture',
         'None', 'None', 'None', '2026-01-02T00:00:00.000Z');
    `);
    expect(
      legacy
        .query<{ held_by: string | null }, []>(
          "SELECT held_by FROM dispatches ORDER BY id",
        )
        .all()
        .map((row) => row.held_by),
    ).toEqual(["cancelled-lane", "open-lane", "returned-lane"]);
    legacy.close();

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const migrated = new Database(databasePath);
    expect(
      migrated
        .query<{ held_by: string | null }, []>(
          "SELECT held_by FROM dispatches ORDER BY id",
        )
        .all()
        .map((row) => row.held_by),
    ).toEqual([null, "open-lane", null]);
    migrated.close();

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const repeated = new Database(databasePath, { readonly: true });
    expect(
      repeated
        .query<{ held_by: string | null }, []>(
          "SELECT held_by FROM dispatches ORDER BY id",
        )
        .all()
        .map((row) => row.held_by),
    ).toEqual([null, "open-lane", null]);
    repeated.close();
  });
});

test("456 dispatch migration backfills openers and refuses untargeted legacy claims", async () => {
  await withFixture(async (fixture) => {
    const evidencedWork = idFrom(
      await runCli(fixture, ["work", "add", "legacy opener evidence", "--atomic-reason", "fixture"]),
    );
    const missingWork = idFrom(
      await runCli(fixture, ["work", "add", "legacy opener missing", "--atomic-reason", "fixture"]),
    );
    const evidenced = dispatchId(await runCli(fixture, dispatchOpenArgs(evidencedWork)));
    const missing = "x2";
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const legacy = new Database(databasePath);
    legacy
      .query(
        `INSERT INTO dispatches
          (id, work_id, objective, owned_scope, excluded_scope, mutation, stop_condition,
           lane, evidence_required, pane, target_session, opened_by, claimed_by, held_by,
           cancelled_at, cancel_reason, created_at, updated_at)
         VALUES (?, ?, 'legacy objective', 'src/plugins/dispatch.ts', 'push',
                 'write-bounded', 'return', 'delivery', 'source', 'w1:pB', NULL, NULL,
                 NULL, NULL, NULL, NULL, ?, ?)`,
      )
      .run(missing, missingWork, new Date().toISOString(), new Date().toISOString());
    legacy.run("ALTER TABLE dispatches DROP COLUMN opened_by");
    legacy.run("ALTER TABLE dispatches DROP COLUMN claimed_by");
    legacy.close();

    const claimant = session("legacy-claimant");
    expect((await runCli(fixture, ["dispatch", "accept", evidenced], claimant)).exitCode).toBe(0);
    expect(
      (
        await runCli(fixture, ["dispatch", "confirm", evidenced, "--session", "legacy-claimant"])
      ).exitCode,
    ).toBe(0);

    const refused = await runCli(fixture, ["dispatch", "accept", missing], claimant);
    expect(refused.exitCode).toBe(1);
    expect(refused.stderr).toContain('"code":"INVALID_STATE"');
    expect(refused.stderr).toContain(
      `maestro dispatch cancel ${missing} --reason legacy-untargeted`,
    );
    expect(refused.stderr).toContain("new dispatch");

    const migrated = new Database(databasePath, { readonly: true });
    try {
      expect(
        migrated
          .query<{ claimed_by: string | null; id: string; opened_by: string | null }, []>(
            "SELECT id, opened_by, claimed_by FROM dispatches ORDER BY id",
          )
          .all(),
      ).toEqual([
        { claimed_by: null, id: evidenced, opened_by: "test-session" },
        { claimed_by: null, id: missing, opened_by: null },
      ]);
    } finally {
      migrated.close();
    }
  });
});

test("176 handback file refuses a status outside the nine-value vocabulary", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const args = handbackFileArgs(dispatch);
    args[args.indexOf("--status") + 1] = "PASS";
    const filed = await runCli(fixture, args);
    expect(filed.exitCode).not.toBe(0);
    for (const status of [
      "DONE",
      "BLOCKED",
      "UNTESTABLE",
      "UNKNOWN",
      "FAILED",
      "CHALLENGE",
      "REOPEN_REQUEST",
      "DEPENDENCY_REQUEST",
      "COUNCIL_REQUEST",
    ]) {
      expect(filed.stderr).toContain(status);
    }
  });
});

test("419 request statuses require a nonblank --request", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    for (const status of [
      "BLOCKED",
      "DEPENDENCY_REQUEST",
      "COUNCIL_REQUEST",
      "REOPEN_REQUEST",
    ]) {
      for (const request of [undefined, "  "]) {
        const args = handbackFileArgs(dispatch);
        args[args.indexOf("--status") + 1] = status;
        if (request !== undefined) args.push("--request", request);
        const filed = await runCli(fixture, args);
        expect(filed.exitCode).not.toBe(0);
        const error = (JSON.parse(filed.stderr) as {
          error: { code: string; field: string; message: string };
        }).error;
        expect(error).toEqual(expect.objectContaining({
          code: "MISSING_ARGUMENT",
          field: "--request",
        }));
        expect(error.message).toContain("--request");
      }
    }
  });
});

test("420 handback show renders retry conditions and requested actions", async () => {
  await withFixture(async (fixture) => {
    for (const [status, label] of [
      ["BLOCKED", "retry when"],
      ["DEPENDENCY_REQUEST", "requested"],
      ["COUNCIL_REQUEST", "requested"],
      ["REOPEN_REQUEST", "requested"],
    ] as const) {
      const dispatch = await openDispatch(fixture);
      await acceptDispatch(fixture, dispatch);
      const request = `${status.toLowerCase()} condition`;
      const args = handbackFileArgs(dispatch);
      args[args.indexOf("--status") + 1] = status;
      args.push("--request", request);
      const filed = await runCli(fixture, args);
      expect(filed.exitCode).toBe(0);
      expect(filed.stdout).toContain(`${label}: ${request}`);

      const shown = await runCli(fixture, ["handback", "show", handbackId(filed)]);
      expect(shown.exitCode).toBe(0);
      expect(shown.stdout).toContain(`${label}: ${request}`);
    }
  });
});

test("177 handback assumptions and residual risks must be explicit while None is valid", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    for (const field of ["--assumptions", "--residual-risks"]) {
      const args = handbackFileArgs(dispatch);
      args[args.indexOf(field) + 1] = "  ";
      const blank = await runCli(fixture, args);
      expect(blank.exitCode).not.toBe(0);
      expect(blank.stderr).toContain(field);
    }

    await acceptDispatch(fixture, dispatch);
    const filed = await runCli(fixture, handbackFileArgs(dispatch));
    expect(filed.exitCode).toBe(0);
    const match = filed.stdout.match(/^(\S+) \[DONE\]/);
    expect(match?.[1]).toBeTruthy();
    const shown = await runCli(fixture, ["handback", "show", match?.[1] as string]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("assumptions not verified: None");
    expect(shown.stdout).toContain("residual risks: None");
    expect(shown.stdout).toContain("incidental findings: None");
  });
});

test("178 handback proof must name an evidence layer", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    const args = handbackFileArgs(dispatch);
    args[args.indexOf("--proof") + 1] = "the check passed";
    const filed = await runCli(fixture, args);
    expect(filed.exitCode).not.toBe(0);
    for (const layer of ["source", "artifact", "installed", "live", "journey"]) {
      expect(filed.stderr).toContain(layer);
    }
  });
});

test("179 a council stays sealed until every dispatch has returned", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const first = await runCli(fixture, handbackFileArgs(council.dispatches[0]));
    expect(first.exitCode).toBe(0);
    const firstHandback = handbackId(first);

    const sealed = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(sealed.exitCode).not.toBe(0);
    expect(sealed.stderr).toContain("SEALED");

    const listed = await runCli(fixture, ["dispatch", "list", council.work]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("council: sealed (1/2 returned)");
    expect(listed.stdout).not.toContain("claim:");

    for (const target of [council.dispatches[0] as string, council.work]) {
      const handbackList = await runCli(fixture, ["handback", "list", target]);
      expect(handbackList.exitCode).toBe(0);
      expect(handbackList.stdout).toContain(`${firstHandback} [SEALED]`);
      expect(handbackList.stdout).not.toContain("the contract is stored");
      const asJson = await runCli(fixture, ["handback", "list", target, "--json"]);
      expect(asJson.stdout).not.toContain("the contract is stored");
      expect(asJson.stdout).not.toContain('"status":"DONE"');
    }
    const scan = await runCli(fixture, ["attention", "--json"]);
    expect(scan.stdout).not.toContain("HANDBACK_UNREVIEWED");

    expect((await runCli(fixture, handbackFileArgs(council.dispatches[1]))).exitCode).toBe(0);
    const opened = await runCli(fixture, ["handback", "show", firstHandback]);
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("claim: the contract is stored");
  });
});

test("180 a single dispatch handback reads immediately", async () => {
  await withFixture(async (fixture) => {
    const dispatch = await openDispatch(fixture);
    await acceptDispatch(fixture, dispatch);
    const filed = await runCli(fixture, handbackFileArgs(dispatch));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["handback", "show", handbackId(filed)]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("claim: the contract is stored");
    expect(shown.stdout).not.toContain("SEALED");
  });
});

test("181 dispatch unseal records its reason and marks later reads unsealed", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const filed = await runCli(fixture, handbackFileArgs(council.dispatches[0]));
    const id = handbackId(filed);
    const reason = "owner ended the council early";

    const unsealed = await runCli(fixture, [
      "dispatch",
      "unseal",
      council.work,
      "--reason",
      reason,
    ]);
    expect(unsealed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["handback", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("council: unsealed (1/2 returned)");
    expect(shown.stdout).toContain(`reason: ${reason}`);

    const trace = await runCli(fixture, ["trace", council.work]);
    expect(trace.exitCode).toBe(0);
    expect(trace.stdout).toContain("dispatch.unseal");
    expect(trace.stdout).toContain(reason);
  });
});

test("182 ready and work show preserve stable work fields while adding dispatches and decisions", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "read path item", "--atomic-reason", "fixture"]),
    );
    const readyBefore = await runCli(fixture, ["ready"], session("lane-ready"));
    const decision = idFrom(
      await runCli(fixture, [
        "decision",
        "draft",
        "keep dispatches attached to work",
        "--work",
        work,
      ]),
    );

    const opened = await runCli(fixture, [
      ...dispatchOpenArgs(work),
      "--target-session",
      "lane-ready",
    ]);
    const dispatch = dispatchId(opened);
    const readyAfter = await runCli(fixture, ["ready"], session("lane-ready"));
    const showAfter = await runCli(fixture, ["work", "show", work]);

    expect(readyAfter.stdout.split("\n")[0]).toBe(readyBefore.stdout.split("\n")[0]);
    expect(readyAfter.stdout).toContain(`dispatch: ${dispatch} [takeable]`);
    expect(showAfter.stdout).toContain(`dispatch: ${dispatch} [open] delivery`);
    expect(showAfter.stdout).toContain(`decision: ${decision} [draft] keep dispatches attached to work`);

    const readyJson = JSON.parse(
      (await runCli(fixture, ["ready", "--json"], session("lane-ready"))).stdout,
    ) as { data: { dispatches: Array<{ id: string }> } };
    expect(readyJson.data.dispatches.map((item) => item.id)).toContain(dispatch);
    const showJson = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(showJson.exitCode).toBe(0);
    const showEnvelope = JSON.parse(showJson.stdout) as {
      data: {
        decisions: Array<{ id: string }>;
        dispatches: Array<{ id: string }>;
        work: { atomicReason: string | null; id: string; kind: string; state: string; title: string };
      };
    };
    expect(showEnvelope.data.work).toEqual(expect.objectContaining({
      atomicReason: "fixture",
      id: work,
      kind: "task",
      state: "open",
      title: "read path item",
    }));
    expect(showEnvelope.data.dispatches.map((item) => item.id)).toContain(dispatch);
    expect(showEnvelope.data.decisions.map((item) => item.id)).toContain(decision);
  });
});

test("183 handoff renders returned handbacks into the packet", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const work = idFrom(
      await runCli(fixture, ["work", "add", "handoff returns", "--atomic-reason", "fixture"]),
    );
    expect(
      (await runCli(fixture, ["bundle", "open", "dispatch-handoff", "--work", work])).exitCode,
    ).toBe(0);
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));
    await acceptDispatch(fixture, dispatch);
    expect((await runCli(fixture, handbackFileArgs(dispatch))).exitCode).toBe(0);

    const handedOff = await runCli(fixture, ["handoff", "dispatch-handoff", "--json"]);
    expect(handedOff.exitCode).toBe(0);
    const envelope = JSON.parse(handedOff.stdout) as {
      data: { handbacks: Array<{ dispatchId: string; status: string }> };
    };
    expect(envelope.data.handbacks).toEqual([
      expect.objectContaining({ dispatchId: dispatch, status: "DONE" }),
    ]);
    const notes = await Bun.file(
      join(fixture.repo, ".maestro", "bundle", "dispatch-handoff", "NOTES.md"),
    ).text();
    expect(notes).toContain("Handbacks:");
    expect(notes).toContain(`[DONE] dispatch ${dispatch}`);
    expect(notes).toContain("claim: the contract is stored");
    expect(notes).toContain("proof: source: focused test passes");
    expect(notes).toContain("assumptions not verified: None");
    expect(notes).toContain("residual risks: None");
    expect(notes).toContain("incidental findings: None");
  });
});

test("184 work done refuses an unreturned dispatch and cancel with a reason unblocks it", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "done gate", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("holder"))).exitCode).toBe(0);
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work)));

    const blocked = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: fixture"],
      session("holder"),
    );
    expect(blocked.exitCode).not.toBe(0);
    const error = JSON.parse(blocked.stderr) as {
      error: { code: string; message: string; origin: string };
    };
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-dispatch");
    expect(error.error.message).toContain(`maestro dispatch cancel ${dispatch} --reason`);

    expect(
      (
        await runCli(fixture, [
          "dispatch",
          "cancel",
          dispatch,
          "--reason",
          "lane was abandoned",
        ])
      ).exitCode,
    ).toBe(0);
    const cancelled = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(cancelled.stdout).toContain(`${dispatch} [cancelled]`);
    expect(cancelled.stdout).toContain("cancel reason: lane was abandoned");
    expect(
      (
        await runCli(
          fixture,
          ["work", "done", work, "--evidence", "source: fixture"],
          session("holder"),
        )
      ).exitCode,
    ).toBe(0);
  });
});

test("185 work start refuses a sealed council until its dispatches are resolved", async () => {
  await withFixture(async (fixture) => {
    const council = await openCouncil(fixture);
    const blocked = await runCli(fixture, ["work", "start", council.work], session("implementer"));
    expect(blocked.exitCode).not.toBe(0);
    const error = JSON.parse(blocked.stderr) as {
      error: { code: string; message: string; origin: string };
    };
    expect(error.error.code).toBe("GATE_BLOCKED");
    expect(error.error.origin).toBe("policy-dispatch");
    expect(error.error.message).toContain("sealed council");

    for (const dispatch of council.dispatches) {
      expect(
        (
          await runCli(fixture, [
            "dispatch",
            "cancel",
            dispatch,
            "--reason",
            "council lane abandoned",
          ])
        ).exitCode,
      ).toBe(0);
    }
    expect(
      (await runCli(fixture, ["work", "start", council.work], session("implementer"))).exitCode,
    ).toBe(0);
  });
});

test("186 attention records one DISPATCH_UNRETURNED packet per fingerprint without routing", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "lead scope", "--kind", "idea"]),
    );
    expect((await runCli(fixture, ["work", "start", parent], session("lead-session"))).exitCode)
      .toBe(0);
    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "lane question",
        "--parent",
        parent,
        "--atomic-reason",
        "fixture",
      ]),
    );
    const opened = await runCli(fixture, [
      ...dispatchOpenArgs(child),
      "--target-session",
      "worker-session",
    ]);
    const dispatch = dispatchId(opened);
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session("worker-session"))).exitCode,
    ).toBe(0);

    const scan = () =>
      runCli(
        fixture,
        ["attention", "--json", "--dispatch-stale", "0.000001"],
        session("scanner-session"),
      );
    const first = await scan();
    expect(first.exitCode).toBe(0);
    const firstEnvelope = JSON.parse(first.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          kind: string;
          packet: string;
          raised: boolean;
          targets?: string[];
        }>;
      };
    };
    const firstDispatch = firstEnvelope.data.detections.filter(
      (finding) => finding.kind === "DISPATCH_UNRETURNED",
    );
    expect(firstDispatch).toHaveLength(1);
    expect(firstDispatch[0]?.raised).toBe(true);
    expect(firstDispatch[0]?.targets).toBeUndefined();
    expect(firstDispatch[0]?.fingerprint).toContain(dispatch);
    expect(firstDispatch[0]?.packet).toContain(`smallest action: maestro dispatch show ${dispatch}`);

    const secondEnvelope = JSON.parse((await scan()).stdout) as typeof firstEnvelope;
    const secondDispatch = secondEnvelope.data.detections.filter(
      (finding) => finding.kind === "DISPATCH_UNRETURNED",
    );
    expect(secondDispatch).toHaveLength(1);
    expect(secondDispatch[0]?.raised).toBe(false);
    expect(secondDispatch[0]?.targets).toBeUndefined();
  });
});

test("264 unreturned dispatch attention distinguishes dead, live, and unknown holders", async () => {
  await withFixture(async (fixture) => {
    const dead = await openDispatch(fixture);
    const live = await openDispatch(fixture);
    const unknown = await openDispatch(fixture);
    expect((await runCli(fixture, ["dispatch", "accept", dead], session("dead-lane"))).exitCode)
      .toBe(0);
    await confirmDispatch(fixture, dead, "dead-lane");
    expect((await runCli(fixture, ["dispatch", "accept", live], session("live-lane"))).exitCode)
      .toBe(0);
    await confirmDispatch(fixture, live, "live-lane");
    expect(
      (await runCli(fixture, ["dispatch", "accept", unknown], session("unknown-lane"))).exitCode,
    ).toBe(0);
    await confirmDispatch(fixture, unknown, "unknown-lane");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    const now = new Date().toISOString();
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', '')`,
      )
      .run("dead-lane", 2_147_483_647, now);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', '')`,
      )
      .run("live-lane", process.pid, now);
    database.close();

    const stale = await runCli(
      fixture,
      ["attention", "--json", "--dispatch-stale", "0.000001"],
      session("scanner-session"),
    );
    expect(stale.exitCode).toBe(0);
    const staleEnvelope = JSON.parse(stale.stdout) as {
      data: {
        detections: Array<{
          fingerprint: string;
          packet: string;
          subjectSession: string | null;
        }>;
      };
    };
    const detections = new Map(
      staleEnvelope.data.detections.map((finding) => [finding.fingerprint, finding]),
    );
    expect(detections.get(`dispatch-unreturned:${dead}`)?.packet).toContain(
      "holder session dead-lane is dead",
    );
    expect(detections.get(`dispatch-unreturned:${live}`)?.packet).toContain(
      "holder session live-lane is live",
    );
    expect(detections.get(`dispatch-unreturned:${unknown}`)?.packet).toContain(
      "unknown: whether the lane is working, blocked, or abandoned",
    );

    const fresh = await runCli(
      fixture,
      ["attention", "--json", "--dispatch-stale", "24"],
      session("scanner-session"),
    );
    expect(fresh.exitCode).toBe(0);
    const freshEnvelope = JSON.parse(fresh.stdout) as typeof staleEnvelope;
    expect(
      freshEnvelope.data.detections
        .filter((finding) => finding.fingerprint.startsWith("dispatch-unreturned:"))
        .map((finding) => finding.subjectSession),
    ).toEqual(["dead-lane"]);
  });
});

test("246 dispatch open stores the pane verbatim and dispatch show reports it", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "pane identity", "--atomic-reason", "fixture"]),
    );
    const pane = "  not/a-herdr-pane::verbatim  ";
    const args = dispatchOpenArgs(work);
    args[args.indexOf("--pane") + 1] = pane;
    const opened = await runCli(fixture, args);
    expect(opened.exitCode).toBe(0);
    const id = dispatchId(opened);

    const shown = await runCli(fixture, ["dispatch", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`pane: ${pane}\n`);

    const json = await runCli(fixture, ["dispatch", "list", work, "--json"]);
    expect(json).toEqual(expect.objectContaining({ exitCode: 0, stderr: "" }));
    const envelope = JSON.parse(json.stdout) as {
      data: { dispatches: Array<{ id: string; pane: string }> };
    };
    expect(envelope.data.dispatches).toContainEqual(expect.objectContaining({ id, pane }));
  });
});

test("285 a returned handback nobody reviewed raises HANDBACK_UNREVIEWED and reaches the hook brief", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "lead scope", "--kind", "idea"]),
    );
    expect((await runCli(fixture, ["work", "start", parent], session("lead-session"))).exitCode)
      .toBe(0);
    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "lane task",
        "--parent",
        parent,
        "--atomic-reason",
        "fixture",
      ]),
    );
    const dispatch = dispatchId(
      await runCli(fixture, [...dispatchOpenArgs(child), "--target-session", "worker-session"]),
    );
    expect(
      (await runCli(fixture, ["dispatch", "accept", dispatch], session("worker-session"))).exitCode,
    ).toBe(0);
    const filed = await runCli(
      fixture,
      [
        "handback",
        "file",
        dispatch,
        "--status",
        "BLOCKED",
        "--request",
        "the mutation scope includes the required boundary",
        "--claim",
        "scope too narrow",
        "--proof",
        "source: dispatch show",
        "--assumptions",
        "None",
        "--residual-risks",
        "None",
        "--incidental-findings",
        "None",
      ],
      session("worker-session"),
    );
    expect(filed.exitCode).toBe(0);
    const handback = filed.stdout.match(/^(h\d+)/)?.[1] as string;

    const scan = () => runCli(fixture, ["attention", "--json"], session("lead-session"));
    const first = JSON.parse((await scan()).stdout) as {
      data: { detections: Array<{ fingerprint: string; kind: string; packet: string }> };
    };
    const unreviewed = first.data.detections.filter((f) => f.kind === "HANDBACK_UNREVIEWED");
    expect(unreviewed).toHaveLength(1);
    expect(unreviewed[0]?.fingerprint).toContain(dispatch);
    expect(unreviewed[0]?.packet).toContain(`${dispatch} returned BLOCKED (${handback})`);
    expect(unreviewed[0]?.packet).toContain(
      "smallest action: re-dispatch on the same pane or cancel",
    );

    const hook = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session("lead-session"),
    );
    expect(hook.exitCode).toBe(0);
    expect(hook.stdout).toContain(`attention HANDBACK_UNREVIEWED dispatch ${dispatch}`);

    const unrelated = await runCli(fixture, [...dispatchOpenArgs(child), "--target-session", "worker-session"]);
    expect(unrelated.exitCode).toBe(0);
    const still = JSON.parse((await scan()).stdout) as typeof first;
    expect(still.data.detections.filter((f) => f.kind === "HANDBACK_UNREVIEWED")).toHaveLength(1);

    const citing = dispatchOpenArgs(child).map((arg) =>
      arg === "Settle the storage boundary" ? `Settle the storage boundary after ${handback}` : arg
    );
    const reopened = await runCli(fixture, [...citing, "--target-session", "worker-session"]);
    expect(reopened.exitCode).toBe(0);
    const second = JSON.parse((await scan()).stdout) as typeof first;
    expect(second.data.detections.filter((f) => f.kind === "HANDBACK_UNREVIEWED")).toHaveLength(0);
    const quiet = await runCli(
      fixture,
      ["hook", "record", "--event", "UserPromptSubmit"],
      session("lead-session"),
    );
    expect(quiet.stdout).not.toContain("attention ");
  });
});

test("421 HANDBACK_UNREVIEWED branches by request status while DONE stays unchanged", async () => {
  await withFixture(async (fixture) => {
    const cases = [
      {
        status: "BLOCKED",
        question: "retry condition met?",
        smallestAction: "re-dispatch on the same pane or cancel",
      },
      {
        status: "DEPENDENCY_REQUEST",
        question: "accept or decline the dependency request?",
        smallestAction: "open the dependency as a work item in the other scope",
      },
      {
        status: "COUNCIL_REQUEST",
        question: "open another council generation or decline?",
        smallestAction: "open a second generation (d688) or decline with a work note",
      },
      {
        status: "REOPEN_REQUEST",
        question: "grant another lease or decline?",
        smallestAction: "grant a new lease or decline",
      },
      {
        status: "DONE",
        question: "close the work, re-dispatch, or cancel?",
        smallestAction: null,
      },
    ] as const;
    const dispatches = new Map<
      string,
      { branch: (typeof cases)[number]; handback: string }
    >();
    for (const branch of cases) {
      const dispatch = await openDispatch(fixture);
      await acceptDispatch(fixture, dispatch);
      const args = handbackFileArgs(dispatch);
      args[args.indexOf("--status") + 1] = branch.status;
      if (branch.status !== "DONE") args.push("--request", `${branch.status} detail`);
      const filed = await runCli(fixture, args);
      expect(filed.exitCode).toBe(0);
      dispatches.set(dispatch, { branch, handback: handbackId(filed) });
    }

    const attention = await runCli(fixture, ["attention", "--json"], session("lead-session"));
    expect(attention.exitCode).toBe(0);
    const detections = (JSON.parse(attention.stdout) as {
      data: { detections: Array<{ fingerprint: string; kind: string; packet: string }> };
    }).data.detections;
    for (const [dispatch, { branch, handback }] of dispatches) {
      const packet = detections.find(
        (detection) => detection.fingerprint === `handback-unreviewed:${dispatch}`,
      )?.packet;
      expect(packet).toBeString();
      expect(packet).toContain(`question: ${branch.question}`);
      expect(packet).toContain(
        `smallest action: ${branch.smallestAction ?? `maestro handback show ${handback}`}`,
      );
      expect(packet).toContain("attention HANDBACK_UNREVIEWED dispatch");
    }
  });
});

test("326 a start rejected by a gate leaves atomic_reason untouched", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(await runCli(fixture, ["work", "add", "atomic rollback target"]));
    expect((await runCli(fixture, dispatchOpenArgs(work))).exitCode).toBe(0);
    expect((await runCli(fixture, dispatchOpenArgs(work))).exitCode).toBe(0);

    const started = await runCli(fixture, ["work", "start", work, "--atomic-reason", "single-file change"]);
    expect(started.exitCode).not.toBe(0);
    expect(started.stderr).toContain("GATE_BLOCKED");

    const db = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const row = db.query("SELECT state, held_by, atomic_reason FROM work WHERE id = ?").get(work) as {
      state: string; held_by: string | null; atomic_reason: string | null;
    };
    db.close();
    expect(row).toEqual({ state: "open", held_by: null, atomic_reason: null });
  });
});

// d706 step 2: the seal is a property of the declared council, so it must not
// move when only the open/return interleaving moves.
async function councilSeal(
  fixture: Fixture,
  order: "open-open-return" | "open-return-open",
): Promise<string> {
  const work = idFrom(
    await runCli(fixture, ["work", "add", `council ${order}`, "--atomic-reason", "fixture"]),
  );
  const first = dispatchId(
    await runCli(fixture, [...dispatchOpenArgs(work), "--council-members", "2"], session("lead")),
  );
  const openSecond = async (): Promise<void> => {
    await runCli(
      fixture,
      [...dispatchOpenArgs(work), "--council-anchor", first],
      session("lead"),
    );
  };
  const returnFirst = async (): Promise<void> => {
    await runCli(fixture, ["dispatch", "accept", first], session("lane-one"));
    await runCli(fixture, ["dispatch", "confirm", first, "--session", "lane-one"], session("lead"));
    await runCli(
      fixture,
      [
        "handback", "file", first, "--status", "DONE",
        "--claim", "first secret view", "--proof", "live: fixture",
        "--assumptions", "None", "--residual-risks", "None", "--incidental-findings", "None",
      ],
      session("lane-one"),
    );
  };
  if (order === "open-open-return") {
    await openSecond();
    await returnFirst();
  } else {
    await returnFirst();
    await openSecond();
  }
  const shown = await runCli(fixture, ["handback", "show", first], session("lead"));
  return shown.exitCode === 0 ? "readable" : "SEALED";
}

test("474 a declared council seals the first view whichever order the lanes are opened and returned (w502, d706)", async () => {
  await withFixture(async (fixture) => {
    expect(await councilSeal(fixture, "open-open-return")).toBe("SEALED");
    expect(await councilSeal(fixture, "open-return-open")).toBe("SEALED");
  });
}, 60_000);

test("475 an undeclared second dispatch is sequential work, not a council (w502, d706)", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "sequential", "--atomic-reason", "fixture"]),
    );
    const first = dispatchId(await runCli(fixture, dispatchOpenArgs(work), session("lead")));
    await runCli(fixture, ["dispatch", "accept", first], session("lane-one"));
    await runCli(fixture, ["dispatch", "confirm", first, "--session", "lane-one"], session("lead"));
    await runCli(
      fixture,
      [
        "handback", "file", first, "--status", "DONE",
        "--claim", "sequential view", "--proof", "live: fixture",
        "--assumptions", "None", "--residual-risks", "None", "--incidental-findings", "None",
      ],
      session("lane-one"),
    );
    await runCli(fixture, dispatchOpenArgs(work), session("lead"));

    const shown = await runCli(fixture, ["handback", "show", first], session("lead"));
    expect(shown.exitCode).toBe(0);
    const listed = await runCli(fixture, ["dispatch", "list", work], session("lead"));
    expect(listed.stdout).not.toContain("council:");
  });
}, 60_000);
