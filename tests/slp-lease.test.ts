import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { Sessions } from "../src/kernel/sessions.ts";
import { Store } from "../src/kernel/store.ts";
import {
  idFrom,
  prepareInstallFixture,
  runCli,
  withFixture,
  type Fixture,
} from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

function dispatchOpenArgs(work: string, targetSession: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return the delivery result",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, publish",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the handback is filed",
    "--lane",
    "delivery",
    "--evidence-required",
    "source and live",
    "--pane",
    "w1:pB",
    "--target-session",
    targetSession,
  ];
}

function dispatchId(result: { stdout: string }): string {
  const match = result.stdout.match(/^(\S+) \[open\]/);
  if (!match?.[1]) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return match[1];
}

function handbackFileArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the delivery result is returned",
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

async function acceptedDispatch(
  fixture: Fixture,
  work: string,
  holder: string,
): Promise<string> {
  const opened = await runCli(fixture, dispatchOpenArgs(work, holder));
  expect(opened.exitCode).toBe(0);
  const dispatch = dispatchId(opened);
  expect((await runCli(fixture, ["dispatch", "accept", dispatch], session(holder))).exitCode)
    .toBe(0);
  return dispatch;
}

test("187 filing a handback releases the filing session's work lease without losing work", async () => {
  await withFixture(async (fixture) => {
    const holder = "filing-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "return delivery", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "note", work, "keep this note"], session(holder))).exitCode,
    ).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET evidence = ? WHERE id = ?").run("source: existing proof", work);
    database.close();
    const dispatch = await acceptedDispatch(fixture, work, holder);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(holder));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(shown.exitCode).toBe(0);
    const envelope = JSON.parse(shown.stdout) as {
      data: {
        notes: Array<{ text: string }>;
        work: { evidence: string | null; heldBy: string | null; state: string };
      };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({
        evidence: "source: existing proof",
        heldBy: null,
        state: "open",
      }),
    );
    expect(envelope.data.notes.map((note) => note.text)).toContain("keep this note");
  });
});

test("188 a non-holder handback leaves another session's work lease alone", async () => {
  await withFixture(async (fixture) => {
    const workHolder = "work-holder";
    const filingSession = "filing-non-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "preserve lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(workHolder))).exitCode).toBe(0);
    const dispatch = await acceptedDispatch(fixture, work, filingSession);

    const filed = await runCli(fixture, handbackFileArgs(dispatch), session(filingSession));
    expect(filed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    const envelope = JSON.parse(shown.stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({ heldBy: workHolder, state: "active" }),
    );
  });
});

test("189 work release refuses a session that does not hold the lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "holder-only release", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("release-holder"))).exitCode)
      .toBe(0);

    const refused = await runCli(fixture, ["work", "release", work], session("other-session"));
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("LEASE_HELD");
    expect(refused.stderr).toContain("release-holder");
    const shown = JSON.parse((await runCli(fixture, ["work", "show", work, "--json"])).stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(shown.data.work).toEqual(
      expect.objectContaining({ heldBy: "release-holder", state: "active" }),
    );
  });
});

test("190 a holder releases open work without losing evidence and records the event", async () => {
  await withFixture(async (fixture) => {
    const holder = "release-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "voluntary release", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE work SET evidence = ? WHERE id = ?").run("source: retained", work);
    database.close();

    const released = await runCli(fixture, ["work", "release", work], session(holder));
    expect(released.exitCode).toBe(0);
    const shown = JSON.parse((await runCli(fixture, ["work", "show", work, "--json"])).stdout) as {
      data: { work: { evidence: string | null; heldBy: string | null; state: string } };
    };
    expect(shown.data.work).toEqual(
      expect.objectContaining({ evidence: "source: retained", heldBy: null, state: "open" }),
    );
    const trace = await runCli(fixture, ["trace", work]);
    expect(trace.stdout).toContain("work.release");
    expect(trace.stdout).toContain(`\"holder\":\"${holder}\"`);
    expect(trace.stdout).not.toContain("work.done");
  });
});

test("191 work reclaim refuses a missing or blank reason", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "reasoned reclaim", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("previous-holder"))).exitCode)
      .toBe(0);

    for (const args of [
      ["work", "reclaim", work],
      ["work", "reclaim", work, "--reason", "   "],
    ]) {
      const refused = await runCli(fixture, args, session("new-holder"));
      expect(refused.exitCode).not.toBe(0);
      expect(refused.stderr).toContain("--reason");
    }
  });
});

test("192 work reclaim records both sessions and the reason without completing work", async () => {
  await withFixture(async (fixture) => {
    const previousHolder = "previous-holder";
    const newHolder = "new-holder";
    const reason = "operator confirmed the prior lane stopped";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "take stopped lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(previousHolder))).exitCode)
      .toBe(0);

    const reclaimed = await runCli(
      fixture,
      ["work", "reclaim", work, "--reason", reason],
      session(newHolder),
    );
    expect(reclaimed.exitCode).toBe(0);
    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    const envelope = JSON.parse(shown.stdout) as {
      data: {
        work: {
          heldBy: string | null;
          reclaimReason: string | null;
          reclaimedBy: string | null;
          reclaimedFrom: string | null;
          state: string;
        };
      };
    };
    expect(envelope.data.work).toEqual(
      expect.objectContaining({
        heldBy: newHolder,
        reclaimReason: reason,
        reclaimedBy: newHolder,
        reclaimedFrom: previousHolder,
        state: "active",
      }),
    );
    expect((await runCli(fixture, ["work", "show", work])).stdout).toContain(
      `reclaim reason: ${reason}`,
    );
    const trace = await runCli(fixture, ["trace", work]);
    expect(trace.stdout).toContain("work.reclaim");
    expect(trace.stdout).toContain(`\"previousHolder\":\"${previousHolder}\"`);
    expect(trace.stdout).toContain(`\"newHolder\":\"${newHolder}\"`);
    expect(trace.stdout).toContain(`\"reason\":\"${reason}\"`);
    expect(trace.stdout).not.toContain("work.done");
  });
});

test("193 a shared-pid holder is stamped once onto TTL, keeps its lease, heartbeats, then expires", async () => {
  await withFixture(async (fixture) => {
    const sharedPid = 1;
    const holder = "shared-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "shared pid lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder, sharedPid))).exitCode)
      .toBe(0);
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', ?)`,
      )
      .run("shared-peer", sharedPid, new Date().toISOString(), fixture.repo);
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(), holder);
    database.close();

    const beforeDowngrade = Date.now();
    const shown = await runCli(fixture, ["work", "show", work, "--json"], session("observer"));
    const afterDowngrade = Date.now();
    const workAfterDowngrade = (JSON.parse(shown.stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    }).data.work;
    expect(workAfterDowngrade).toEqual(
      expect.objectContaining({ heldBy: holder, state: "active" }),
    );
    database = new Database(databasePath);
    const stamped = database
      .query<{ anchor: string; last_seen: string }, [string]>(
        "SELECT anchor, last_seen FROM sessions WHERE id = ?",
      )
      .get(holder);
    expect(stamped?.anchor).toBe("ttl");
    expect(Date.parse(stamped?.last_seen ?? "")).toBeGreaterThanOrEqual(beforeDowngrade);
    expect(Date.parse(stamped?.last_seen ?? "")).toBeLessThanOrEqual(afterDowngrade);

    const halfWindow = new Date(Date.now() - 30 * 60 * 1000).toISOString();
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?").run(halfWindow, holder);
    database.close();
    const beforeHeartbeat = Date.now();
    expect(
      (await runCli(fixture, ["work", "note", work, "heartbeat command"], session(holder, sharedPid)))
        .exitCode,
    ).toBe(0);
    database = new Database(databasePath);
    const heartbeat = database
      .query<{ anchor: string; last_seen: string }, [string]>(
        "SELECT anchor, last_seen FROM sessions WHERE id = ?",
      )
      .get(holder);
    expect(heartbeat?.anchor).toBe("ttl");
    expect(Date.parse(heartbeat?.last_seen ?? "")).toBeGreaterThanOrEqual(beforeHeartbeat);

    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(new Date(Date.now() - 61 * 60 * 1000).toISOString(), holder);
    database.close();
    const expired = JSON.parse(
      (await runCli(fixture, ["work", "show", work, "--json"], session("observer"))).stdout,
    ) as { data: { work: { heldBy: string | null; state: string } } };
    expect(expired.data.work).toEqual(expect.objectContaining({ heldBy: null, state: "open" }));
  });
});

test("194 an exclusive live pid stays pid-anchored even with a cold last_seen", async () => {
  await withFixture(async (fixture) => {
    const holder = "exclusive-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "exclusive pid lease", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder, 1))).exitCode).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    const cold = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString();
    database.query("UPDATE sessions SET last_seen = ? WHERE id = ?").run(cold, holder);
    database.close();

    const shown = JSON.parse(
      (await runCli(fixture, ["work", "show", work, "--json"], session("observer"))).stdout,
    ) as { data: { work: { heldBy: string | null; state: string } } };
    expect(shown.data.work).toEqual(
      expect.objectContaining({ heldBy: holder, state: "active" }),
    );
    const readback = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const anchor = readback
      .query<{ anchor: string; last_seen: string }, [string]>(
        "SELECT anchor, last_seen FROM sessions WHERE id = ?",
      )
      .get(holder);
    readback.close();
    expect(anchor).toEqual({ anchor: "pid", last_seen: cold });
  });
});

test("195 repeated work reads write a shared-pid downgrade exactly once", async () => {
  await withFixture(async (fixture) => {
    const sharedPid = 1;
    const holder = "one-write-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "one downgrade", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder, sharedPid))).exitCode)
      .toBe(0);
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', ?)`,
      )
      .run("one-write-peer", sharedPid, new Date().toISOString(), fixture.repo);
    database.exec(`
      CREATE TABLE downgrade_audit (session_id TEXT NOT NULL);
      CREATE TRIGGER count_holder_downgrade
      AFTER UPDATE OF anchor ON sessions
      WHEN OLD.id = '${holder}' AND OLD.anchor = 'pid' AND NEW.anchor = 'ttl'
      BEGIN
        INSERT INTO downgrade_audit(session_id) VALUES (NEW.id);
      END;
    `);
    database.close();

    for (const command of [
      ["work", "show", work],
      ["work", "show", work],
      ["work", "list"],
      ["ready"],
    ]) {
      expect((await runCli(fixture, command, session("observer"))).exitCode).toBe(0);
    }
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
          session(holder, sharedPid),
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "show", work], session("observer"))).exitCode).toBe(0);
    database = new Database(databasePath, { readonly: true });
    const writes = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM downgrade_audit")
      .get()?.count;
    const anchor = database
      .query<{ anchor: string }, [string]>("SELECT anchor FROM sessions WHERE id = ?")
      .get(holder)?.anchor;
    database.close();
    expect(writes).toBe(1);
    expect(anchor).toBe("ttl");
  });
});

test("196 doctor counts the mailbox target that Sessions.liveness calls dead", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    for (const [id, pid] of [
      // A live host process: the shared pid is ambiguous, not dead.
      ["shared-target", 1],
      ["shared-peer", 1],
      ["ttl-dead", 99_999_998],
    ] as const) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
            session(id, pid),
          )
        ).exitCode,
      ).toBe(0);
    }
    expect(
      (await runCli(fixture, ["msg", "send", "shared-target", "shared queued"])).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["msg", "send", "ttl-dead", "ttl queued"])).exitCode)
      .toBe(0);

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(databasePath);
    const stale = new Date(Date.now() - 61 * 60 * 1000).toISOString();
    database
      .query("UPDATE sessions SET anchor = 'pid', last_seen = ? WHERE id IN (?, ?)")
      .run(stale, "shared-target", "shared-peer");
    database
      .query("UPDATE sessions SET anchor = 'ttl', last_seen = ? WHERE id = ?")
      .run(stale, "ttl-dead");
    database.close();

    const doctor = await runCli(fixture, ["doctor"], { PATH: path });
    expect(doctor.exitCode).toBe(0);

    const store = new Store(databasePath);
    try {
      const sessions = new Sessions(store, fixture.repo);
      expect(sessions.liveness("shared-target").live).toBe(true);
      expect(sessions.liveness("ttl-dead").live).toBe(false);
    } finally {
      store.close();
    }
    expect(doctor.stdout).toContain("mailbox: 1 message(s) queued for dead sessions");
  });
});

test("197 a shared pid that is already dead downgrades without inventing liveness", async () => {
  await withFixture(async (fixture) => {
    const deadPid = 99_999_997;
    const holder = "dead-shared-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "dead shared pid lease", "--atomic-reason", "fixture"]),
    );
    // The lease is taken while the host process is alive; the pid dies later.
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const cold = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString();
    let database = new Database(databasePath);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', ?)`,
      )
      .run("dead-shared-peer", deadPid, cold, fixture.repo);
    database
      .query("UPDATE sessions SET pid = ?, anchor = 'pid', last_seen = ? WHERE id = ?")
      .run(deadPid, cold, holder);
    database.close();

    const shown = await runCli(fixture, ["work", "show", work, "--json"], session("observer"));
    const workAfterDowngrade = (JSON.parse(shown.stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    }).data.work;
    expect(workAfterDowngrade).toEqual(expect.objectContaining({ heldBy: null, state: "open" }));

    database = new Database(databasePath);
    const stamped = database
      .query<{ anchor: string; last_seen: string }, [string]>(
        "SELECT anchor, last_seen FROM sessions WHERE id = ?",
      )
      .get(holder);
    database.close();
    // A shared pid stops proving life, but a dead pid still proves death: the
    // anchor moves and the recorded clock is left where it was.
    expect(stamped).toEqual({ anchor: "ttl", last_seen: cold });
  });
});
