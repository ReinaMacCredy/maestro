import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  prepareInstallFixture,
  runCli,
  withFixture,
  type Fixture,
} from "./helpers.ts";

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
  // A live row needs a host that was running when it was last seen: pid 1,
  // seen two hours ago or just after boot on a machine younger than that.
  const booted = Date.parse(
    Bun.spawnSync(["ps", "-o", "lstart=", "-p", "1"], { env: { LC_ALL: "C" } }).stdout.toString().trim(),
  );
  const lastSeen = new Date(Math.max(booted + 1000, Date.now() - 2 * 60 * 60_000)).toISOString();
  database
    .query(
      "INSERT INTO sessions (id, pid, last_event, last_seen, harness) VALUES (?, ?, 'SessionStart', ?, 'codex')",
    )
    .run("legacy-live", 1, lastSeen);
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

test("200 read-only status treats pre-anchor session rows as PID-anchored", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    await recreatePreAnchorSessionStore(fixture);

    const diagnosed = await runCli(fixture, ["doctor"], { PATH: path });
    expect(diagnosed.exitCode).toBe(0);
    const brief = await runCli(fixture, ["brief", "--session"], {
      MAESTRO_READ_ONLY: "1",
    });
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain("live peers:\n- legacy-live holds: none");
    expect(brief.stdout).not.toContain("legacy-dead");
  });
});

test("352 [closeout-only] doctor and read-only status do not migrate the pre-anchor schema", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const { databasePath, schemaBefore } = await recreatePreAnchorSessionStore(fixture);

    expect((await runCli(fixture, ["doctor"], { PATH: path })).exitCode).toBe(0);
    expect(
      (await runCli(fixture, ["brief", "--session"], { MAESTRO_READ_ONLY: "1" })).exitCode,
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
