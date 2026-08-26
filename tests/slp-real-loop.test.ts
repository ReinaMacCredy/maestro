import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
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

function dispatchOpenArgs(work: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Return the detector result",
    "--owned-scope",
    "scratch",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "the lane reports",
    "--lane",
    "delivery",
    "--evidence-required",
    "journey",
    "--pane",
    "w1:pC",
    "--target-session",
    "worker-session",
  ];
}

async function terminalWork(fixture: Fixture, terminal: "cancelled" | "done"): Promise<string> {
  const work = idFrom(
    await runCli(fixture, [
      "work",
      "add",
      `${terminal} dispatch subject`,
      "--atomic-reason",
      "real loop fixture",
    ]),
  );
  if (terminal === "done") {
    expect((await runCli(fixture, ["work", "start", work])).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "done", work, "--evidence", "source: fixture"]))
        .exitCode,
    ).toBe(0);
  } else {
    expect(
      (await runCli(fixture, ["work", "cancel", work, "--reason", "fixture terminal state"]))
        .exitCode,
    ).toBe(0);
  }
  return work;
}

test("198 unreturned dispatch attention ignores done and cancelled work", async () => {
  await withFixture(async (fixture) => {
    // d30 closed the door these rows came through, so the detector's suppression
    // is now only reachable by rows written before that rule existed.
    const dispatches: string[] = [];
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    try {
      for (const [index, terminal] of (["done", "cancelled"] as const).entries()) {
        const work = await terminalWork(fixture, terminal);
        const id = `legacy-x${index + 1}`;
        database
          .query(
            `INSERT INTO dispatches
              (id, work_id, objective, owned_scope, excluded_scope, mutation, stop_condition,
               lane, evidence_required, target_session, created_at, updated_at)
             VALUES (?, ?, 'o', 's', 'e', 'no-write', 'the lane reports', 'delivery', 'journey',
                     'worker-session', ?, ?)`,
          )
          .run(id, work, "2026-01-01T00:00:00.000Z", "2026-01-01T00:00:00.000Z");
        dispatches.push(id);
      }
    } finally {
      database.close();
    }

    const scanned = await runCli(fixture, [
      "attention",
      "--json",
      "--dispatch-stale",
      "0.000001",
    ]);
    expect(scanned.exitCode).toBe(0);
    const envelope = JSON.parse(scanned.stdout) as {
      data: { detections: Array<{ fingerprint: string; kind: string }> };
    };
    expect(
      envelope.data.detections.filter(
        (finding) =>
          finding.kind === "DISPATCH_UNRETURNED" &&
          dispatches.some((dispatch) => finding.fingerprint.includes(dispatch)),
      ),
    ).toEqual([]);
  });
});

test("199 doctor computes shared-pid liveness without mutating the store", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    for (const id of ["shared-target", "shared-peer"]) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
            { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: "1" },
          )
        ).exitCode,
      ).toBe(0);
    }
    expect(
      (await runCli(fixture, ["msg", "send", "shared-target", "queued message"])).exitCode,
    ).toBe(0);

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const cold = new Date(Date.now() - 2 * 60 * 60_000).toISOString();
    let database = new Database(databasePath);
    database
      .query("UPDATE sessions SET anchor = 'pid', last_seen = ? WHERE id IN (?, ?)")
      .run(cold, "shared-target", "shared-peer");
    const before = database
      .query("SELECT id, anchor, last_seen FROM sessions ORDER BY id")
      .all();
    const schemaBefore = database
      .query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .all();
    database.close();

    const diagnosed = await runCli(fixture, ["doctor"], { PATH: path });
    expect(diagnosed.exitCode).toBe(0);
    expect(diagnosed.stdout).toContain("mailbox: ok");

    const inspectedBefore = Date.now();
    const readOnlyStore = new Store(databasePath, { readonly: true });
    const inspected = new Sessions(readOnlyStore, fixture.repo).get("shared-target");
    readOnlyStore.close();
    expect(inspected?.anchor).toBe("ttl");
    expect(inspected?.live).toBe(true);
    expect(Date.parse(inspected?.lastSeen ?? "")).toBeGreaterThanOrEqual(inspectedBefore);

    database = new Database(databasePath, { readonly: true });
    const after = database
      .query("SELECT id, anchor, last_seen FROM sessions ORDER BY id")
      .all();
    const schemaAfter = database
      .query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .all();
    database.close();
    expect(after).toEqual(before);
    expect(schemaAfter).toEqual(schemaBefore);
  });
});

test("200 doctor treats a pre-anchor session row as PID-anchored without migrating it", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    await Promise.all([
      rm(databasePath, { force: true }),
      rm(`${databasePath}-shm`, { force: true }),
      rm(`${databasePath}-wal`, { force: true }),
    ]);
    const database = new Database(databasePath, { create: true });
    database.exec(`
      CREATE TABLE sessions (
        id TEXT PRIMARY KEY,
        pid INTEGER NOT NULL,
        last_event TEXT NOT NULL,
        last_seen TEXT NOT NULL,
        harness TEXT
      );
      CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        sender_session TEXT NOT NULL,
        target_session TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE TABLE message_cursors (
        session_id TEXT PRIMARY KEY,
        last_message_id INTEGER NOT NULL DEFAULT 0
      );
    `);
    database
      .query(
        "INSERT INTO sessions (id, pid, last_event, last_seen, harness) VALUES (?, 1, 'SessionStart', ?, 'codex')",
      )
      .run("legacy-live", new Date(Date.now() - 2 * 60 * 60_000).toISOString());
    database
      .query(
        "INSERT INTO messages (sender_session, target_session, text, created_at) VALUES ('sender', ?, 'queued', ?)",
      )
      .run("legacy-live", new Date().toISOString());
    const schemaBefore = database
      .query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .all();
    database.close();

    const diagnosed = await runCli(fixture, ["doctor"], { PATH: path });
    expect(diagnosed.exitCode).toBe(0);
    expect(diagnosed.stdout).toContain("mailbox: ok");
    expect(diagnosed.stdout).not.toContain("queued for dead sessions");

    const verified = new Database(databasePath, { readonly: true });
    const schemaAfter = verified
      .query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .all();
    const columns = verified
      .query<{ name: string }, []>("PRAGMA table_info(sessions)")
      .all()
      .map((column) => column.name);
    verified.close();
    expect(schemaAfter).toEqual(schemaBefore);
    expect(columns).not.toContain("anchor");
  });
});

test("201 dispatch open refuses work that is already done or cancelled", async () => {
  await withFixture(async (fixture) => {
    for (const terminal of ["done", "cancelled"] as const) {
      const work = await terminalWork(fixture, terminal);
      const opened = await runCli(fixture, dispatchOpenArgs(work));
      expect(opened.exitCode).not.toBe(0);
      expect(opened.stderr).toContain("INVALID_STATE");
      expect(opened.stderr).toContain(terminal);

      const listed = await runCli(fixture, ["dispatch", "list", work]);
      expect(listed.stdout).not.toContain("[open]");
    }
  });
});

test("202 work cancel is gated by an open dispatch exactly as work done is", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "gated cancel subject", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "start", work], { MAESTRO_SESSION_ID: "lead" })).exitCode)
      .toBe(0);
    const opened = await runCli(fixture, dispatchOpenArgs(work), { MAESTRO_SESSION_ID: "lead" });
    expect(opened.exitCode).toBe(0);
    const dispatchId = opened.stdout.trim().split(/\s+/)[0] as string;

    const blocked = await runCli(
      fixture,
      ["work", "cancel", work, "--reason", "abandoning the item"],
      { MAESTRO_SESSION_ID: "lead" },
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(blocked.stderr).toContain("GATE_BLOCKED");
    expect(blocked.stderr).toContain(`maestro dispatch cancel ${dispatchId} --reason`);
    expect((await runCli(fixture, ["work", "show", work])).stdout).toContain("[active]");

    expect(
      (await runCli(fixture, ["dispatch", "cancel", dispatchId, "--reason", "lane abandoned"], {
        MAESTRO_SESSION_ID: "lead",
      })).exitCode,
    ).toBe(0);
    const cancelled = await runCli(
      fixture,
      ["work", "cancel", work, "--reason", "abandoning the item"],
      { MAESTRO_SESSION_ID: "lead" },
    );
    expect(cancelled.exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "show", work])).stdout).toContain("[cancelled]");
  });
});
