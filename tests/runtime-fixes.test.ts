import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

const storeCardId = "feature-runtime-store";
const archiveCardId = "snapshot-runtime-archive";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

function dispatchOpenArgs(work: string, target: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "Verify the runtime contract",
    "--owned-scope",
    "src/plugins/dispatch.ts",
    "--excluded-scope",
    "push, tag, release",
    "--mutation",
    "write-bounded: src/plugins/dispatch.ts",
    "--stop-condition",
    "the runtime assertion passes",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: focused test",
    "--pane",
    "w1:p-test",
    "--target-session",
    target,
  ];
}

function dispatchId(stdout: string): string {
  const match = stdout.match(/^x\d+/);
  if (!match) throw new Error(`missing dispatch id in stdout: ${stdout}`);
  return match[0];
}

function handbackArgs(dispatch: string): string[] {
  return [
    "handback",
    "file",
    dispatch,
    "--status",
    "DONE",
    "--claim",
    "the runtime contract is verified",
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

async function writeStoreSource(fixture: Fixture): Promise<string> {
  const path = join(fixture.repo, "runtime-store.sqlite");
  const database = new Database(path, { create: true, strict: true });
  database.exec(`
    CREATE TABLE cards (
      id TEXT PRIMARY KEY NOT NULL,
      card_type TEXT NOT NULL,
      parent TEXT,
      status TEXT NOT NULL,
      title TEXT NOT NULL,
      record_file TEXT NOT NULL,
      card_yaml TEXT NOT NULL,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      imported_at TEXT NOT NULL
    );
    CREATE TABLE card_files (
      card_id TEXT NOT NULL,
      path TEXT NOT NULL,
      mode INTEGER NOT NULL,
      contents BLOB NOT NULL,
      sha256 TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY (card_id, path)
    );
    INSERT INTO cards
      (id, card_type, parent, status, title, record_file, card_yaml,
       created_at, updated_at, imported_at)
    VALUES
      ('${storeCardId}', 'feature', NULL, 'closed', 'Runtime Store Card', 'card.yaml',
       'id: ${storeCardId}\ncard_type: feature\nstatus: closed\ntitle: Runtime Store Card\ndescription: store-runtime-needle\n',
       '2026-08-27T00:00:00Z', '2026-08-27T00:00:00Z', '2026-08-27T00:00:00Z');
  `);
  database.close();
  return path;
}

async function writeArchiveSource(fixture: Fixture): Promise<string> {
  const path = join(fixture.repo, "runtime-archive.sqlite");
  const database = new Database(path, { create: true, strict: true });
  database.exec(`
    CREATE TABLE archived_snapshots (
      id TEXT PRIMARY KEY NOT NULL,
      archived_at TEXT NOT NULL,
      source_relpath TEXT NOT NULL,
      manifest_json TEXT NOT NULL,
      snapshot_zstd BLOB NOT NULL,
      snapshot_sha256 TEXT NOT NULL,
      search_text TEXT NOT NULL,
      last_checked_at TEXT
    );
  `);
  database
    .query(
      `INSERT INTO archived_snapshots
        (id, archived_at, source_relpath, manifest_json, snapshot_zstd,
         snapshot_sha256, search_text, last_checked_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .run(
      archiveCardId,
      "2026-08-27T00:01:00Z",
      `archive/${archiveCardId}`,
      JSON.stringify({ format_version: "maestro.archive.snapshot.v1", files: [] }),
      new Uint8Array([0]),
      "runtime-archive-sha",
      "title: Runtime Archive Snapshot\ndescription: archive-runtime-needle\n",
    );
  database.close();
  return path;
}

test("320 import rust keeps store and archive sources independently replaceable", async () => {
  await withFixture(async (fixture) => {
    const store = await writeStoreSource(fixture);
    const archive = await writeArchiveSource(fixture);

    expect((await runCli(fixture, ["import", "rust", "--path", store])).exitCode).toBe(0);
    expect((await runCli(fixture, ["import", "rust", "--path", archive])).exitCode).toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
      strict: true,
    });
    expect(
      database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
        .get()?.count,
    ).toBe(2);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT count(*) AS count FROM legacy_cards WHERE card_type = 'archive'",
        )
        .get()?.count,
    ).toBe(1);
    expect(
      database
        .query<{ source: string }, []>("SELECT DISTINCT source FROM legacy_cards ORDER BY source")
        .all()
        .map((row) => row.source),
    ).toEqual(["archive", "store"]);
    database.close();

    expect((await runCli(fixture, ["legacy", "show", storeCardId])).exitCode).toBe(0);
    expect((await runCli(fixture, ["legacy", "show", archiveCardId])).exitCode).toBe(0);
    expect((await runCli(fixture, ["search", "store-runtime-needle"])).stdout).toContain(storeCardId);
    expect((await runCli(fixture, ["search", "archive-runtime-needle"])).stdout).toContain(
      archiveCardId,
    );

    expect((await runCli(fixture, ["import", "rust", "--path", archive])).exitCode).toBe(0);
    const afterRepeat = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
      strict: true,
    });
    expect(
      afterRepeat
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
        .get()?.count,
    ).toBe(2);
    afterRepeat.close();

    const promoted = await runCli(fixture, ["import", "rust", "--path", store, "--promote"]);
    const rerun = await runCli(fixture, ["import", "rust", "--path", store, "--promote"]);
    expect(promoted.exitCode).toBe(0);
    expect(rerun.exitCode).toBe(0);
    expect(rerun.stdout).toContain("0 work created");
    expect(rerun.stdout).toContain("0 decisions created");
    expect((await runCli(fixture, ["legacy", "show", archiveCardId])).exitCode).toBe(0);
  });
});

test("321 handback keeps the work lease until the owning session completes work", async () => {
  await withFixture(async (fixture) => {
    const owner = session("runtime-owner");
    const other = session("runtime-other");
    const work = idFrom(
      await runCli(
        fixture,
        ["work", "add", "handback lease", "--atomic-reason", "fixture"],
        owner,
      ),
    );
    expect((await runCli(fixture, ["work", "start", work], owner)).exitCode).toBe(0);
    const opened = await runCli(fixture, dispatchOpenArgs(work, "runtime-owner"), owner);
    const dispatch = dispatchId(opened.stdout);
    expect(opened.exitCode).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", dispatch], owner)).exitCode).toBe(0);
    expect((await runCli(fixture, handbackArgs(dispatch), owner)).exitCode).toBe(0);

    const refused = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: wrong session"],
      other,
    );
    expect(refused.exitCode).not.toBe(0);
    expect(JSON.parse(refused.stderr).error.code).toBe("LEASE_HELD");

    const completed = await runCli(
      fixture,
      ["work", "done", work, "--evidence", "source: owner session"],
      owner,
    );
    expect(completed.exitCode).toBe(0);
  });
});

test("322 help --help prints top-level help without changing existing help forms", async () => {
  await withFixture(async (fixture) => {
    const topLevel = await runCli(fixture, ["help"]);
    const flagged = await runCli(fixture, ["help", "--help"]);
    const perVerb = await runCli(fixture, ["help", "work"]);

    expect(topLevel.exitCode).toBe(0);
    expect(flagged.exitCode).toBe(0);
    expect(flagged.stdout).toBe(topLevel.stdout);
    expect(flagged.stdout).toContain("verbs:");
    expect(perVerb.exitCode).toBe(0);
    expect(perVerb.stdout).toContain("usage: maestro work");
  });
});

test("323 cancelled dispatches leave the live council before work start", async () => {
  await withFixture(async (fixture) => {
    const owner = session("council-owner");
    const work = idFrom(
      await runCli(
        fixture,
        ["work", "add", "cancelled council lane", "--atomic-reason", "fixture"],
        owner,
      ),
    );
    const first = dispatchId(
      (await runCli(fixture, dispatchOpenArgs(work, "council-owner"), owner)).stdout,
    );
    const second = dispatchId(
      (await runCli(fixture, dispatchOpenArgs(work, "council-owner"), owner)).stdout,
    );

    expect(
      (
        await runCli(
          fixture,
          ["dispatch", "cancel", first, "--reason", "lane replaced"],
          owner,
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["dispatch", "accept", second], owner)).exitCode).toBe(0);

    const started = await runCli(fixture, ["work", "start", work], owner);
    expect(started.exitCode).toBe(0);
    const listed = await runCli(fixture, ["dispatch", "list", work], owner);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).not.toContain("council:");
  });
});
