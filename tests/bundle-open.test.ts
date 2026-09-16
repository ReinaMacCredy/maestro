import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { installFakeHerdr } from "./helpers-herdr.ts";
import { initializeGitRepository, runCli, runCliAt, withFixture } from "./helpers.ts";

// Hub finding F4 (w734): bundle open --work checked the non-SLP work table
// only, so an id the project store holds as an slp_work row - every id an SLP
// generation issues - failed NOT_FOUND. An id in either table is accepted; the
// stored bundle_work link keeps its shape.

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

function failure(stderr: string): { code: string; message: string } {
  return (JSON.parse(stderr) as { error: { code: string; message: string } }).error;
}

test("bundle open --work accepts an SLP work id and still refuses an unknown one (Hub finding F4)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const room = await scaffoldRoom(fixture.home);
    expect(
      (await runCliAt(fixture, room, ["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1", MAESTRO_SESSION_NONE: "1" }))
        .exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Bundle link", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const slpWorkId = envelope<{ work: { id: string } }>(started.stdout).work.id;

    const opened = await runCli(fixture, ["bundle", "open", "slp-linked", "--work", slpWorkId, "--json"]);
    expect(opened.stderr).toBe("");
    expect(opened.exitCode).toBe(0);
    expect(envelope<{ workIds: string[] }>(opened.stdout).workIds).toEqual([slpWorkId]);

    const missing = await runCli(fixture, ["bundle", "open", "absent-link", "--work", "w9999", "--json"]);
    expect(missing.exitCode).toBe(1);
    expect(failure(missing.stderr).code).toBe("NOT_FOUND");
  });
}, 30_000);

test("bundle open migrates a legacy bundle_work away from the work(id) foreign key, keeping its rows (Hub finding F4)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const added = await runCli(fixture, ["work", "add", "plain item", "--atomic-reason", "fixture", "--json"]);
    expect(added.exitCode).toBe(0);
    const workId = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect((await runCli(fixture, ["bundle", "open", "legacy-link", "--work", workId])).exitCode).toBe(0);

    // Put the pre-F4 schema back, rows and all, so the migration has something
    // to rebuild the way a store written before this change holds it.
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    let database = new Database(databasePath);
    database.exec("PRAGMA foreign_keys = OFF");
    database.exec("ALTER TABLE bundle_work RENAME TO bundle_work_old");
    database.exec(
      `CREATE TABLE bundle_work (
         bundle_id TEXT NOT NULL REFERENCES bundles(id),
         work_id TEXT NOT NULL REFERENCES work(id),
         PRIMARY KEY(bundle_id, work_id)
       )`,
    );
    database.exec("INSERT INTO bundle_work SELECT * FROM bundle_work_old");
    database.exec("DROP TABLE bundle_work_old");
    expect(
      database.query<{ sql: string }, []>("SELECT sql FROM sqlite_master WHERE name = 'bundle_work'").get()!.sql,
    ).toContain("REFERENCES work(id)");
    database.close();

    const shown = await runCli(fixture, ["bundle", "show", "legacy-link", "--json"]);
    expect(shown.exitCode).toBe(0);

    database = new Database(databasePath, { readonly: true });
    const schema = database
      .query<{ sql: string }, []>("SELECT sql FROM sqlite_master WHERE name = 'bundle_work'")
      .get()!.sql;
    const rows = database.query<{ bundle_id: string; work_id: string }, []>("SELECT * FROM bundle_work").all();
    database.close();
    expect(schema).not.toContain("REFERENCES work(id)");
    expect(schema).toContain("REFERENCES bundles(id)");
    expect(schema).toContain("PRIMARY KEY(bundle_id, work_id)");
    expect(rows).toEqual([{ bundle_id: "legacy-link", work_id: workId }]);
  });
}, 30_000);
