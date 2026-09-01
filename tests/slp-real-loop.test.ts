import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
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

async function recreatePreAnchorSessionStore(
  fixture: Fixture,
): Promise<{ databasePath: string; schemaBefore: unknown[] }> {
  const databasePath = join(fixture.repo, ".maestro", "maestro.db");
  const database = new Database(databasePath);
  database.exec(`
    DROP TABLE sessions;
    CREATE TABLE sessions (
      id TEXT PRIMARY KEY,
      pid INTEGER NOT NULL,
      last_event TEXT NOT NULL,
      last_seen TEXT NOT NULL,
      harness TEXT
    );
  `);
  const lastSeen = new Date(Date.now() - 2 * 60 * 60_000).toISOString();
  database
    .query(
      "INSERT INTO sessions (id, pid, last_event, last_seen, harness) VALUES (?, ?, 'SessionStart', ?, 'codex')",
    )
    .run("legacy-live", process.pid, lastSeen);
  database
    .query(
      "INSERT INTO sessions (id, pid, last_event, last_seen, harness) VALUES (?, ?, 'SessionStart', ?, 'codex')",
    )
    .run("legacy-dead", 2_147_483_647, lastSeen);
  const schemaBefore = database
    .query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
    .all();
  database.close();
  return { databasePath, schemaBefore };
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

test("200 read-only status treats pre-anchor session rows as PID-anchored", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    await recreatePreAnchorSessionStore(fixture);

    const diagnosed = await runCli(fixture, ["doctor"], { PATH: path });
    expect(diagnosed.exitCode).toBe(0);
    const all = await runCli(fixture, ["status", "--all", "--json"], {
      MAESTRO_READ_ONLY: "1",
    });
    const live = await runCli(fixture, ["status", "--live", "--json"], {
      MAESTRO_READ_ONLY: "1",
    });
    expect(all.exitCode).toBe(0);
    expect(live.exitCode).toBe(0);
    const allSessions = (JSON.parse(all.stdout) as {
      data: { sessions: Array<{ id: string; live: boolean }> };
    }).data.sessions;
    const liveSessions = (JSON.parse(live.stdout) as {
      data: { sessions: Array<{ id: string; live: boolean }> };
    }).data.sessions;
    expect(allSessions).toContainEqual(expect.objectContaining({ id: "legacy-live", live: true }));
    expect(allSessions).toContainEqual(expect.objectContaining({ id: "legacy-dead", live: false }));
    expect(liveSessions).toContainEqual(expect.objectContaining({ id: "legacy-live", live: true }));
    expect(liveSessions.some((session) => session.id === "legacy-dead")).toBe(false);
  });
});

test("352 [closeout-only] doctor and read-only status do not migrate the pre-anchor schema", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const { databasePath, schemaBefore } = await recreatePreAnchorSessionStore(fixture);

    expect((await runCli(fixture, ["doctor"], { PATH: path })).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["status", "--json"], { MAESTRO_READ_ONLY: "1" })).exitCode,
    ).toBe(0);

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
    expect(columns).not.toContain("scope");
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
