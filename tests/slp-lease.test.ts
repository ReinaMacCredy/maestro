import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  runCli,
  withFixture,
  workEvents,
} from "./helpers.ts";

function session(id: string, pid = process.pid): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: String(pid),
  };
}

test("189 work release refuses a session that does not hold the lease", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "holder-only release"]),
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
      await runCli(fixture, ["work", "add", "voluntary release"]),
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
    const trace = workEvents(fixture, work);
    expect(trace).toContain("work.release");
    expect(trace).toContain(`\"holder\":\"${holder}\"`);
    expect(trace).not.toContain("work.done");
  });
});

test("652 work release --force drops another live session's lease and records who forced it", async () => {
  await withFixture(async (fixture) => {
    const holder = "stuck-holder";
    const forcer = "forcing-session";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "forced release"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder))).exitCode).toBe(0);

    const forced = await runCli(fixture, ["work", "release", work, "--force"], session(forcer));
    expect(forced.exitCode).toBe(0);
    expect(forced.stdout).toContain(`lease dropped from ${holder}`);
    const shown = JSON.parse((await runCli(fixture, ["work", "show", work, "--json"])).stdout) as {
      data: { work: { heldBy: string | null; state: string } };
    };
    expect(shown.data.work).toEqual(expect.objectContaining({ heldBy: null, state: "open" }));
    const trace = workEvents(fixture, work);
    expect(trace).toContain("work.release");
    expect(trace).toContain(`\"holder\":\"${holder}\"`);
    expect(trace).toContain(`\"forcedBy\":\"${forcer}\"`);

    const nothing = await runCli(fixture, ["work", "release", work, "--force"], session(forcer));
    expect(nothing.exitCode).not.toBe(0);
    expect(nothing.stderr).toContain("LEASE_REQUIRED");
  });
});

test("191 work reclaim refuses a missing or blank reason", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "reasoned reclaim"]),
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
      await runCli(fixture, ["work", "add", "take stopped lease"]),
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
    const trace = workEvents(fixture, work);
    expect(trace).toContain("work.reclaim");
    expect(trace).toContain(`\"previousHolder\":\"${previousHolder}\"`);
    expect(trace).toContain(`\"newHolder\":\"${newHolder}\"`);
    expect(trace).toContain(`\"reason\":\"${reason}\"`);
    expect(trace).not.toContain("work.done");
  });
});

test("193 a shared-pid holder is stamped once onto TTL, keeps its lease, heartbeats, then expires", async () => {
  await withFixture(async (fixture) => {
    const sharedPid = 1;
    const holder = "shared-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "shared pid lease"]),
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
      await runCli(fixture, ["work", "add", "exclusive pid lease"]),
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
      await runCli(fixture, ["work", "add", "one downgrade"]),
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
      ["work", "list", "--all"],
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

test("197 a shared pid that is already dead downgrades without inventing liveness", async () => {
  await withFixture(async (fixture) => {
    const deadPid = 99_999_997;
    const holder = "dead-shared-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "dead shared pid lease"]),
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

test("295 shared-pid downgrade never decreases a newer persisted heartbeat", async () => {
  await withFixture(async (fixture) => {
    const sharedPid = 1;
    const holder = "monotonic-holder";
    const work = idFrom(
      await runCli(fixture, ["work", "add", "monotonic downgrade"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session(holder, sharedPid))).exitCode)
      .toBe(0);
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const newerHeartbeat = new Date(Date.now() + 60_000).toISOString();
    let database = new Database(databasePath);
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, 'SessionStart', ?, 'codex', 'pid', ?)`,
      )
      .run("monotonic-peer", sharedPid, new Date().toISOString(), fixture.repo);
    database
      .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
      .run(newerHeartbeat, holder);
    database.close();

    expect((await runCli(fixture, ["work", "show", work], session("observer"))).exitCode).toBe(0);
    database = new Database(databasePath, { readonly: true });
    const persisted = database
      .query<{ anchor: string; last_seen: string }, [string]>(
        "SELECT anchor, last_seen FROM sessions WHERE id = ?",
      )
      .get(holder);
    database.close();
    expect(persisted).toEqual({ anchor: "ttl", last_seen: newerHeartbeat });
  });
});

test("470 a replacement process reusing a pid gets a fresh session and cannot finish the old lease (w472/d704)", async () => {
  await withFixture(async (fixture) => {
    // A live pid: the old session must still look alive, or the lease would
    // simply expire and the reuse would never be tested.
    const reused = process.pid;
    const work = idFrom(
      await runCli(fixture, ["work", "add", "pid reuse repro"]),
    );
    expect(
      (await runCli(
        fixture,
        ["hook", "record", "--event", "SessionStart"],
        session("old-session", reused),
      )).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "start", work], session("old-session", reused))).exitCode)
      .toBe(0);

    // The replacement carries only the pid: no explicit identity anywhere.
    const blank = {
      MAESTRO_SESSION_ID: "",
      MAESTRO_SESSION_PID: String(reused),
      CODEX_SESSION_ID: "",
      CODEX_THREAD_ID: "",
      CLAUDE_CODE_SESSION_ID: "",
      CLAUDE_SESSION_ID: "",
      CURSOR_SESSION_ID: "",
    };
    const completed = await runCli(
      fixture,
      ["work", "done", work, "--claim", "inherited", "--proof", "same pid only"],
      blank,
    );
    expect(completed.exitCode).not.toBe(0);
    expect(completed.stderr).toContain("old-session");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const done = database
      .query<{ count: number }, [string]>(
        "SELECT COUNT(*) AS count FROM event_log WHERE type = 'work.done' AND entity_id = ?",
      )
      .get(work);
    const shown = database
      .query<{ held_by: string | null; state: string }, [string]>(
        "SELECT held_by, state FROM work WHERE id = ?",
      )
      .get(work);
    database.close();
    expect(done?.count).toBe(0);
    expect(shown).toEqual({ held_by: "old-session", state: "active" });
  });
});

test("471 the current session id does not depend on which historical row shares its pid (w472/d704)", async () => {
  await withFixture(async (fixture) => {
    const blank = {
      MAESTRO_SESSION_ID: "",
      MAESTRO_SESSION_PID: String(process.pid),
      CODEX_SESSION_ID: "",
      CODEX_THREAD_ID: "",
      CLAUDE_CODE_SESSION_ID: "",
      CLAUDE_SESSION_ID: "",
      CURSOR_SESSION_ID: "",
    };
    const identity = async (title: string): Promise<string> => {
      const work = idFrom(
        await runCli(fixture, ["work", "add", title], blank),
      );
      const started = await runCli(fixture, ["work", "start", work], blank);
      expect(started.exitCode).toBe(0);
      return started.stdout.trim().split(" started by ")[1] ?? "missing session";
    };

    const before = await identity("before the perturbation");

    // Perturbation: plant a prior session row on the very same pid. The
    // resolved identity must not move because of it.
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES ('planted', ?, 'SessionStart', ?, 'codex', 'pid', ?)`,
      )
      .run(process.pid, new Date().toISOString(), fixture.repo);
    database.close();

    const after = await identity("after the perturbation");

    expect(before).not.toBe("planted");
    expect(after).not.toBe("planted");
    expect(after).not.toBe(before);
  });
});

test("480 work add alone records a live session before attributing its event", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(
      fixture,
      ["work", "add", "add-only session"],
      session("add-only"),
    );
    expect(added.exitCode).toBe(0);
    const work = idFrom(added);

    expect((await runCli(fixture, ["brief", "--session"])).stdout).toContain("\n- add-only holds:");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const attribution = database
      .query<
        { id: string; last_event: string; session_id: string },
        [string]
      >(
        `SELECT sessions.id, sessions.last_event, event_log.session_id
         FROM event_log
         JOIN sessions ON sessions.id = event_log.session_id
         WHERE event_log.type = 'work.add' AND event_log.entity_id = ?`,
      )
      .get(work);
    database.close();
    expect(attribution).toEqual({
      id: "add-only",
      last_event: "work.add",
      session_id: "add-only",
    });
  });
});

test("540 work done claims an unheld card and completes it in one command (w548/d720)", async () => {
  await withFixture(async (fixture) => {
    // A relay card is never worked in the delivery sense: it records a handoff
    // and closes when the report arrives, so the start carried no information.
    const work = idFrom(
      await runCli(fixture, ["work", "add", "relay card"]),
    );
    const done = await runCli(
      fixture,
      ["work", "done", work, "--claim", "relayed: lead reported done", "--proof", "source: report"],
      session("relay-holder"),
    );
    expect(done.exitCode).toBe(0);
    expect(done.stdout).toContain(`${work} done`);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const events = database
      .query<{ payload: string; type: string }, [string]>(
        "SELECT payload, type FROM event_log WHERE entity_id = ?",
      )
      .all(work);
    const row = database
      .query<{ held_by: string | null; state: string }, [string]>(
        "SELECT held_by, state FROM work WHERE id = ?",
      )
      .get(work);
    database.close();
    expect(row).toEqual({ held_by: null, state: "done" });
    const types = events.map((event) => event.type);
    expect(types).toContain("work.done");
    expect(types).not.toContain("work.start");
    // The implicit claim is recorded where the completion is, not as a
    // zero-length start no reader consumes.
    const completion = events.find((event) => event.type === "work.done");
    expect(JSON.parse(completion?.payload ?? "{}").claimedOnDone).toBe(true);
  });
});

test("541 work done still refuses a card another live session holds (w548/d720)", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "held card"]),
    );
    expect((await runCli(fixture, ["work", "start", work], session("first-holder"))).exitCode).toBe(0);

    const stolen = await runCli(
      fixture,
      ["work", "done", work, "--claim", "took it", "--proof", "source: none"],
      session("second-session"),
    );
    expect(stolen.exitCode).not.toBe(0);
    expect(stolen.stderr).toContain("LEASE_HELD");
    expect(stolen.stderr).toContain("first-holder");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const row = database
      .query<{ held_by: string | null; state: string }, [string]>(
        "SELECT held_by, state FROM work WHERE id = ?",
      )
      .get(work);
    database.close();
    expect(row).toEqual({ held_by: "first-holder", state: "active" });
  });
});

test("542 the implicit claim on work done answers to blockers (w548/d720)", async () => {
  await withFixture(async (fixture) => {
    const blocker = idFrom(
      await runCli(fixture, ["work", "add", "blocker"]),
    );
    const blocked = idFrom(
      await runCli(
        fixture,
        ["work", "add", "blocked card", "--blocked-by", blocker],
      ),
    );

    const done = await runCli(
      fixture,
      ["work", "done", blocked, "--claim", "skipped ahead", "--proof", "source: none"],
      session("blocked-session"),
    );
    expect(done.exitCode).not.toBe(0);
    expect(done.stderr).toContain("BLOCKED");
    expect(done.stderr).toContain(blocker);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const row = database
      .query<{ held_by: string | null; state: string }, [string]>(
        "SELECT held_by, state FROM work WHERE id = ?",
      )
      .get(blocked);
    database.close();
    expect(row).toEqual({ held_by: null, state: "open" });
  });
});
