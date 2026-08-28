import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

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

test("519 a store records the schema generation that wrote it", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["work", "list"])).exitCode).toBe(0);
    expect(userVersion(storePath(fixture))).toBeGreaterThan(0);
  });
});

test("520 an older maestro refuses a store a newer one wrote, rather than writing into it", async () => {
  await withFixture(async (fixture) => {
    await runCli(fixture, ["work", "add", "before the upgrade", "--atomic-reason", "fixture"]);
    const path = storePath(fixture);
    const current = userVersion(path);
    // The store now claims a schema this binary has never heard of, which is
    // what a stale shim or a downgrade sees after someone else upgrades.
    setUserVersion(path, current + 1);

    const write = await runCli(fixture, ["work", "add", "after", "--atomic-reason", "fixture"]);
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
    await runCli(fixture, ["work", "add", "kept", "--atomic-reason", "fixture"]);
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
    await runCli(fixture, ["work", "add", "seed", "--atomic-reason", "fixture"]);
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
