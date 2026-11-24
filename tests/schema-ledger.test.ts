import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { rm } from "node:fs/promises";
import { join } from "node:path";
import { enableSlp, runCli, withFixture, type Fixture } from "./helpers.ts";

function storePath(fixture: Fixture): string {
  return join(fixture.repo, ".maestro", "maestro.db");
}

function userVersion(path: string): number {
  const database = new Database(path, { readonly: true });
  try {
    return database.query<{ user_version: number }, []>("PRAGMA user_version").get()?.user_version ?? -1;
  } finally {
    database.close();
  }
}

function setUserVersion(path: string, value: number): void {
  const database = new Database(path);
  try {
    database.exec(`PRAGMA user_version = ${value}`);
  } finally {
    database.close();
  }
}

test("628 a store records the schema generation that wrote it", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["work", "list"])).exitCode).toBe(0);
    expect(userVersion(storePath(fixture))).toBeGreaterThan(0);
  });
});

test("520 an older maestro refuses a store a newer one wrote, rather than writing into it", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["work", "add", "before the upgrade"]);
    const path = storePath(fixture);
    const current = userVersion(path);
    // The store now claims a schema this binary has never heard of, which is
    // what a stale shim or a downgrade sees after someone else upgrades.
    setUserVersion(path, current + 1);

    const write = await runCli(fixture, ["work", "add", "after"]);
    const read = await runCli(fixture, ["work", "list"]);

    for (const result of [write, read]) {
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("STORE_TOO_NEW");
      expect(result.stderr).toContain("maestro update");
    }
    // Refusing means refusing: the newer store keeps its own generation.
    expect(userVersion(path)).toBe(current + 1);
  });
});

test("521 a store from an older generation is stamped forward and keeps working", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["work", "add", "kept"]);
    const path = storePath(fixture);
    const current = userVersion(path);
    // Every store written before the ledger existed reads as 0.
    setUserVersion(path, 0);

    const listed = await runCli(fixture, ["work", "list"]);

    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain("kept");
    expect(userVersion(path)).toBe(current);
  });
});

test("522 observer mode refuses a newer store and stamps nothing", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["work", "add", "seed"]);
    const path = storePath(fixture);
    const current = userVersion(path);
    setUserVersion(path, 0);

    const observed = await runCli(fixture, ["work", "list"], { MAESTRO_READ_ONLY: "1" });
    expect(observed.exitCode).toBe(0);
    // A read must not migrate the store it is only looking at.
    expect(userVersion(path)).toBe(0);

    setUserVersion(path, current + 1);
    const refused = await runCli(fixture, ["work", "list"], { MAESTRO_READ_ONLY: "1" });
    expect(refused.exitCode).not.toBe(0);
    expect(refused.stderr).toContain("STORE_TOO_NEW");
  });
});

function withDatabase(path: string, run: (database: Database) => void): void {
  const database = new Database(path);
  try {
    run(database);
  } finally {
    database.close();
  }
}

async function createdId(fixture: Fixture, args: string[]): Promise<string> {
  const result = await runCli(fixture, [...args, "--json"]);
  expect({ code: result.exitCode, stderr: result.stderr }).toMatchObject({ code: 0 });
  const data = (JSON.parse(result.stdout) as { data: { decision?: { id: string }; work?: { id: string } } }).data;
  return (data.work ?? data.decision)!.id;
}

const addWork = (fixture: Fixture, title: string) =>
  createdId(fixture, ["work", "add", title]);
const draftDecision = (fixture: Fixture, text: string) => createdId(fixture, ["decision", "draft", text]);

test("d894 ids count 1, 2, 3 in a fresh store", async () => {
  await withFixture(async (fixture) => {
    expect([await addWork(fixture, "a"), await addWork(fixture, "b"), await addWork(fixture, "c")])
      .toEqual(["w1", "w2", "w3"]);
    expect([await draftDecision(fixture, "a"), await draftDecision(fixture, "b"), await draftDecision(fixture, "c")])
      .toEqual(["d1", "d2", "d3"]);
  });
});

test("d894 plain cards and decisions draw above the SLP ledgers, and the counter never goes back", async () => {
  await withFixture(async (fixture) => {
    await addWork(fixture, "plain");
    const path = storePath(fixture);
    withDatabase(path, (database) => {
      database.exec("CREATE TABLE slp_work (id TEXT PRIMARY KEY)");
      database.exec("CREATE TABLE slp_decisions (id TEXT PRIMARY KEY)");
      database.exec("INSERT INTO slp_work VALUES ('w7')");
      database.exec("INSERT INTO slp_decisions VALUES ('d9')");
    });

    expect(await addWork(fixture, "above slp")).toBe("w8");
    expect(await draftDecision(fixture, "above slp")).toBe("d10");

    withDatabase(path, (database) => {
      database.exec("DELETE FROM work WHERE id = 'w8'");
      database.exec("DELETE FROM slp_work");
      database.exec("DELETE FROM decisions WHERE id = 'd10'");
      database.exec("DELETE FROM slp_decisions");
    });
    expect(await addWork(fixture, "after delete")).toBe("w9");
    expect(await draftDecision(fixture, "after delete")).toBe("d11");
  });
});

test("d894 a store whose ledgers already share ids shows them as before, and opens read-only without the counter", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "plain card");
    const decision = await draftDecision(fixture, "plain decision");
    const path = storePath(fixture);
    withDatabase(path, (database) => {
      database.exec("CREATE TABLE slp_work (id TEXT PRIMARY KEY)");
      database.exec(
        `CREATE TABLE slp_decisions (id TEXT PRIMARY KEY, team_id TEXT, generation INTEGER, choice TEXT,
          why TEXT, scope TEXT, work_id TEXT, replaces_id TEXT, actor TEXT, created_at TEXT)`,
      );
      database.query("INSERT INTO slp_work VALUES (?)").run(work);
      database
        .query(
          "INSERT INTO slp_decisions VALUES (?, 't', 1, 'slp ruling', 'why', 'team', NULL, NULL, 'lead', '2026-01-01T00:00:00Z')",
        )
        .run(decision);
      database.exec("DROP TABLE IF EXISTS id_counters");
    });

    const observedWork = await runCli(fixture, ["work", "show", work], { MAESTRO_READ_ONLY: "1" });
    expect(observedWork.exitCode).toBe(0);
    expect(observedWork.stdout).toContain("plain card");
    const pluginOff = await runCli(fixture, ["decision", "show", decision], { MAESTRO_READ_ONLY: "1" });
    expect(pluginOff.exitCode).toBe(0);
    expect(pluginOff.stdout).toContain("plain decision");
    expect(pluginOff.stdout).toContain(`${decision} [team] by lead\nchoice: slp ruling\nwhy: why`);
    expect(pluginOff.stdout).toContain("maestro plugin enable slp");
    await enableSlp(fixture);
    const observedDecision = await runCli(fixture, ["decision", "show", decision], { MAESTRO_READ_ONLY: "1" });
    expect(observedDecision.exitCode).toBe(0);
    expect(observedDecision.stdout).toContain("plain decision");
    expect(observedDecision.stdout).toContain("slp ruling");
    expect(observedDecision.stdout).not.toContain("maestro plugin enable slp");
    await rm(join(fixture.home, ".maestro", "config"));
    withDatabase(path, (database) => {
      expect(database.query("SELECT name FROM sqlite_master WHERE name = 'id_counters'").get()).toBeNull();
    });

    expect(await addWork(fixture, "next")).toBe("w2");
    expect(await draftDecision(fixture, "next")).toBe("d2");
  });
});
