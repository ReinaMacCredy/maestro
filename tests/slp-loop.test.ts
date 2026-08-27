import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  idFrom,
  runCli,
  type Fixture,
  withFixture,
} from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

async function addWork(fixture: Fixture, title: string): Promise<string> {
  return idFrom(
    await runCli(fixture, ["work", "add", title, "--atomic-reason", "slp loop fixture"]),
  );
}

async function addFailedNotes(fixture: Fixture, workId: string): Promise<void> {
  for (const text of ["failed: first", "failed: second", "failed: third"]) {
    expect((await runCli(fixture, ["work", "note", workId, text])).exitCode).toBe(0);
  }
}

function loopDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"));
}

function insertLegacyCard(database: Database, id: string, title: string): void {
  const now = "2026-08-26T00:00:00Z";
  database
    .query(
      `INSERT INTO legacy_cards
        (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
       VALUES (?, 'feature', NULL, 'open', ?, 'card.yaml', ?, ?, ?, ?)`,
    )
    .run(id, title, `id: ${id}\ntitle: ${title}\n`, now, now, now);
}

test("153 repeated-failure attention skips terminal work and retains open and active work", async () => {
  await withFixture(async (fixture) => {
    const done = await addWork(fixture, "terminal done");
    const cancelled = await addWork(fixture, "terminal cancelled");
    const open = await addWork(fixture, "still open");
    const active = await addWork(fixture, "still active");

    expect((await runCli(fixture, ["work", "start", done], session("done-holder"))).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["work", "start", cancelled], session("cancel-holder"))).exitCode,
    ).toBe(0);
    expect(
      (await runCli(fixture, ["work", "start", active], session("active-holder"))).exitCode,
    ).toBe(0);
    for (const workId of [done, cancelled, open, active]) await addFailedNotes(fixture, workId);

    expect(
      (
        await runCli(
          fixture,
          ["work", "done", done, "--claim", "terminal done", "--proof", "notes recorded"],
          session("done-holder"),
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCli(
          fixture,
          ["work", "cancel", cancelled, "--reason", "terminal cancelled"],
          session("cancel-holder"),
        )
      ).exitCode,
    ).toBe(0);

    const result = await runCli(fixture, ["attention", "--json"], session("scanner"));
    expect(result.exitCode).toBe(0);
    const output = JSON.parse(result.stdout) as {
      data: { detections: Array<{ kind: string; subjectWork: string }> };
    };
    expect(
      output.data.detections
        .filter((finding) => finding.kind === "REPEATED_FAILURE")
        .map((finding) => finding.subjectWork),
    ).toEqual([open, active]);
  });
});

test("154 a current search backfill version preserves sentinel rows across boots", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const database = loopDatabase(fixture);
    try {
      database
        .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('sentinel', 'keep', 'keep sentinel')")
        .run();
    } finally {
      database.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const verified = loopDatabase(fixture);
    try {
      expect(
        verified
          .query<{ count: number }, []>(
            "SELECT count(*) AS count FROM search_index WHERE surface = 'sentinel' AND entity_id = 'keep'",
          )
          .get()?.count,
      ).toBe(1);
    } finally {
      verified.close();
    }
  });
});

test("155 a stale search backfill version rebuilds exactly the next boot", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const database = loopDatabase(fixture);
    try {
      database.query("UPDATE search_index_state SET version = 0").run();
      database
        .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('sentinel', 'once', 'rebuild once')")
        .run();
    } finally {
      database.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const afterRebuild = loopDatabase(fixture);
    try {
      expect(
        afterRebuild
          .query<{ count: number }, []>(
            "SELECT count(*) AS count FROM search_index WHERE surface = 'sentinel' AND entity_id = 'once'",
          )
          .get()?.count,
      ).toBe(0);
      expect(
        afterRebuild.query<{ version: number }, []>("SELECT version FROM search_index_state").get()
          ?.version,
      ).toBe(1);
      afterRebuild
        .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('sentinel', 'once', 'rebuild once')")
        .run();
    } finally {
      afterRebuild.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const stable = loopDatabase(fixture);
    try {
      expect(
        stable
          .query<{ count: number }, []>(
            "SELECT count(*) AS count FROM search_index WHERE surface = 'sentinel' AND entity_id = 'once'",
          )
          .get()?.count,
      ).toBe(1);
    } finally {
      stable.close();
    }
  });
});

test("156 legacy search restores missing rows once and preserves existing rowids", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const database = loopDatabase(fixture);
    let originalRowid = 0;
    try {
      insertLegacyCard(database, "legacy-boot", "Legacy boot sentinel");
      database
        .query(
          "INSERT INTO search_index(rowid, surface, entity_id, text) VALUES (900001, '[legacy]', 'legacy-boot', 'Legacy boot sentinel')",
        )
        .run();
      originalRowid = database
        .query<{ rowid: number }, []>(
          "SELECT rowid FROM search_index WHERE surface = '[legacy]' AND entity_id = 'legacy-boot'",
        )
        .get()?.rowid ?? 0;
    } finally {
      database.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const present = loopDatabase(fixture);
    try {
      expect(originalRowid).toBeGreaterThan(0);
      expect(
        present
          .query<{ rowid: number }, []>(
            "SELECT rowid FROM search_index WHERE surface = '[legacy]' AND entity_id = 'legacy-boot'",
          )
          .get()?.rowid,
      ).toBe(originalRowid);
      present
        .query("DELETE FROM search_index WHERE surface = '[legacy]' AND entity_id = 'legacy-boot'")
        .run();
    } finally {
      present.close();
    }

    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);
    const restored = loopDatabase(fixture);
    try {
      expect(
        restored
          .query<{ count: number }, []>(
            "SELECT count(*) AS count FROM search_index WHERE surface = '[legacy]' AND entity_id = 'legacy-boot'",
          )
          .get()?.count,
      ).toBe(1);
    } finally {
      restored.close();
    }
  });
});

test("157 search still finds fresh native surfaces and a legacy card", async () => {
  await withFixture(async (fixture) => {
    const work = idFrom(
      await runCli(fixture, ["work", "add", "freshworktoken", "--atomic-reason", "fixture"]),
    );
    expect((await runCli(fixture, ["work", "note", work, "freshnotetoken"])).exitCode).toBe(0);
    expect(
      (
        await runCli(fixture, [
          "decision",
          "draft",
          "freshdecisiontoken",
          "--rationale",
          "fixture",
          "--work",
          work,
        ])
      ).exitCode,
    ).toBe(0);
    expect(
      (await runCli(fixture, ["hook", "record", "--event", "freshlogtoken"])).exitCode,
    ).toBe(0);

    const database = loopDatabase(fixture);
    try {
      insertLegacyCard(database, "legacy-search", "freshlegacytoken");
      database.query("DELETE FROM search_index WHERE surface = '[legacy]'").run();
    } finally {
      database.close();
    }
    expect((await runCli(fixture, ["version"])).exitCode).toBe(0);

    for (const [term, expected] of [
      ["freshworktoken", work],
      ["freshnotetoken", work],
      ["freshdecisiontoken", "decision"],
      ["freshlogtoken", "hook.record"],
      ["freshlegacytoken", "[legacy] legacy-search"],
    ] satisfies Array<[string, string]>) {
      const result = await runCli(fixture, ["search", term]);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain(expected);
    }
  });
});

test("158 attention records one packet independently of live peer order", async () => {
  await withFixture(async (fixture) => {
    const parent = await addWork(fixture, "unheld attention parent");
    const child = idFrom(
      await runCli(fixture, [
        "work",
        "add",
        "attention child",
        "--parent",
        parent,
        "--kind",
        "task",
      ]),
    );
    expect((await runCli(fixture, ["work", "start", child], session("attention-subject"))).exitCode)
      .toBe(0);
    for (const peer of ["peer-old", "peer-new", "peer-middle"]) {
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart"],
            session(peer),
          )
        ).exitCode,
      ).toBe(0);
    }
    const database = loopDatabase(fixture);
    try {
      const now = Date.now();
      for (const [id, minutes] of [
        ["attention-subject", 45],
        ["peer-old", 3],
        ["peer-new", 1],
        ["peer-middle", 2],
      ] satisfies Array<[string, number]>) {
        database
          .query("UPDATE sessions SET last_seen = ? WHERE id = ?")
          .run(new Date(now - minutes * 60_000).toISOString(), id);
      }
    } finally {
      database.close();
    }

    expect((await runCli(fixture, ["attention"], session("attention-scanner"))).exitCode).toBe(0);
    const verified = loopDatabase(fixture);
    try {
      expect(verified.query<{ count: number }, []>("SELECT count(*) AS count FROM attention").get()?.count)
        .toBe(1);
    } finally {
      verified.close();
    }
  });
});
