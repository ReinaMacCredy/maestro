import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";

function searchRows(path: string): Array<{ rowid: number; surface: string; entity_id: string }> {
  const database = new Database(path, { readwrite: true });
  try {
    return database
      .query<{ rowid: number; surface: string; entity_id: string }, []>(
        "SELECT rowid, surface, entity_id FROM search_index WHERE surface IN ('term', 'bundle') ORDER BY rowid",
      )
      .all();
  } finally {
    database.close();
  }
}

function seed(path: string): void {
  const database = new Database(path, { readwrite: true });
  try {
    const now = new Date().toISOString();
    database
      .query("INSERT INTO terms(id, name, definition, work_id, created_at, updated_at) VALUES ('t1', 'quillon', 'the crossguard of a sword', NULL, ?, ?)")
      .run(now, now);
    database.run("INSERT INTO search_index(surface, entity_id, text) VALUES ('term', 't1', 'quillon the crossguard of a sword')");
    database
      .query("INSERT INTO bundles(id, state, directory, spec, notes, verify, created_at, updated_at) VALUES ('forge', 'archived', '/nowhere', 'pommel spec', 'notes', 'verify', ?, ?)")
      .run(now, now);
    database.run("INSERT INTO search_index(surface, entity_id, text) VALUES ('bundle', 'forge', 'forge\npommel spec\nnotes\nverify')");
  } finally {
    database.close();
  }
}

test("an invocation leaves the search rows of terms and archived bundles as they were written", async () => {
  await withFixture(async (fixture) => {
    const store = join(fixture.repo, ".maestro", "maestro.db");
    expect((await runCli(fixture, ["work", "add", "seed the store"])).exitCode).toBe(0);
    seed(store);
    const before = searchRows(store);
    expect(before.map((row) => row.surface)).toEqual(["term", "bundle"]);

    expect((await runCli(fixture, ["work", "list"])).exitCode).toBe(0);

    expect(searchRows(store)).toEqual(before);
  });
});

test("a search index rebuild keeps the rows of terms and archived bundles findable", async () => {
  await withFixture(async (fixture) => {
    const store = join(fixture.repo, ".maestro", "maestro.db");
    expect((await runCli(fixture, ["work", "add", "seed the store"])).exitCode).toBe(0);
    seed(store);
    const database = new Database(store, { readwrite: true });
    database.run("UPDATE search_index_state SET version = 0");
    database.close();

    const term = await runCli(fixture, ["search", "quillon", "--local"]);
    const bundle = await runCli(fixture, ["search", "pommel", "--local"]);
    const work = await runCli(fixture, ["search", "seed", "--local"]);

    expect(term.stdout).toContain("t1");
    expect(bundle.stdout).toContain("forge");
    expect(work.stdout).toContain("w1");
    expect(searchRows(store)).toHaveLength(2);
  });
});
