import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

const featureId = "feature-legacy-aurora";

function sha256(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function writeLegacyStore(fixture: Fixture, extraCards = 0): Promise<string> {
  const path = join(fixture.repo, ".maestro", "store.sqlite");
  const database = new Database(path, { create: true, strict: true });
  database.exec(`
    CREATE TABLE schema_version (version INTEGER NOT NULL);
    INSERT INTO schema_version(version) VALUES (1);
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
      PRIMARY KEY (card_id, path),
      FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
    );
  `);
  const insertCard = database.query(
    `INSERT INTO cards
      (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
     VALUES (?, ?, ?, ?, ?, 'card.yaml', ?, ?, ?, ?)`,
  );
  const now = "2026-08-24T00:00:00Z";
  insertCard.run(
    featureId,
    "feature",
    null,
    "closed",
    "Legacy Aurora",
    `schema_version: "4"\nid: ${featureId}\ncard_type: feature\nstatus: closed\ntitle: Legacy Aurora\ndescription: Historic constellation plan\n`,
    now,
    now,
    now,
  );
  insertCard.run(
    "dec-root-sqlite",
    "decision",
    null,
    "locked",
    "Keep SQLite archive",
    `schema_version: "4"\nid: dec-root-sqlite\ncard_type: decision\nstatus: locked\ntitle: Keep SQLite archive\ndecision: Preserve the quorum history\n`,
    now,
    now,
    now,
  );
  insertCard.run(
    "dec-aurora-color",
    "decision",
    featureId,
    "locked",
    "Choose violet telemetry",
    `schema_version: "4"\nid: dec-aurora-color\ncard_type: decision\nparent: ${featureId}\nstatus: locked\ntitle: Choose violet telemetry\ndecision: Use ultraviolet signals\n`,
    now,
    now,
    now,
  );
  for (let index = 0; index < extraCards; index += 1) {
    const id = `feature-context-bomb-${String(index).padStart(2, "0")}`;
    insertCard.run(
      id,
      "feature",
      null,
      "closed",
      `Context bomb ${index}`,
      `schema_version: "4"\nid: ${id}\ncard_type: feature\nstatus: closed\ntitle: Context bomb ${index}\ndescription: ${"contextbomb ".repeat(80)}\n`,
      now,
      now,
      now,
    );
  }

  const insertFile = database.query(
    `INSERT INTO card_files (card_id, path, mode, contents, sha256, updated_at)
     VALUES (?, ?, 420, ?, ?, ?)`,
  );
  const design = "# Aurora design\nThe quasar archive remains searchable.\n";
  const decisions =
    "- id: dec-aurora-color\n  title: Choose violet telemetry\n  status: locked\n  decision: Use ultraviolet signals\n";
  const binary = new Uint8Array([0xff, 0xfe, 0x00, 0x80]);
  insertFile.run(featureId, "design.md", design, sha256(design), now);
  insertFile.run(featureId, "decisions.yaml", decisions, sha256(decisions), now);
  insertFile.run(featureId, "diagram.bin", binary, sha256(binary), now);
  database.close();
  return path;
}

function targetDatabase(fixture: Fixture): Database {
  return new Database(join(fixture.repo, ".maestro", "maestro.db"), {
    readonly: true,
    strict: true,
  });
}

test("37 import rust copies legacy cards, text files, decisions, and preserves its source", async () => {
  await withFixture(async (fixture) => {
    const source = await writeLegacyStore(fixture);
    const before = sha256(await readFile(source));

    const imported = await runCli(fixture, ["import", "rust"]);

    expect(imported.exitCode).toBe(0);
    expect(imported.stdout).toContain("3 cards");
    expect(imported.stdout).toContain("3 files");
    expect(imported.stdout).toContain("2 decisions");
    expect(sha256(await readFile(source))).toBe(before);

    const shown = await runCli(fixture, ["legacy", "show", featureId]);
    expect(shown.stdout).toContain("design.md");
    expect(shown.stdout).toContain("The quasar archive remains searchable.");
    expect(shown.stdout).toContain(`diagram.bin sha256=${sha256(new Uint8Array([0xff, 0xfe, 0x00, 0x80]))} size=4`);
    expect(shown.stdout).not.toContain("�");
  });
});

test("38 search returns tagged legacy title, file, and decision hits alongside native hits", async () => {
  await withFixture(async (fixture) => {
    await writeLegacyStore(fixture);
    const native = await runCli(fixture, ["work", "add", "Native Aurora follow-up"]);
    expect(native.exitCode).toBe(0);
    expect((await runCli(fixture, ["import", "rust"])).exitCode).toBe(0);

    const title = await runCli(fixture, ["search", "Aurora"]);
    const file = await runCli(fixture, ["search", "quasar"]);
    const decision = await runCli(fixture, ["search", "quorum"]);
    const perCardDecision = await runCli(fixture, ["search", "ultraviolet"]);

    expect(title.stdout).toContain(`work ${idFrom(native)}`);
    expect(title.stdout).toContain(`work ${idFrom(native)}: Native Aurora follow-up`);
    expect(title.stdout).toContain(`[legacy] ${featureId}`);
    expect(file.stdout).toContain(`[legacy] ${featureId}`);
    expect(decision.stdout).toContain("[legacy] dec-root-sqlite");
    expect(perCardDecision.stdout).toContain(`[legacy] ${featureId}`);
  });
});

test("39 legacy show prints one card, selects one file, and suggests a near miss", async () => {
  await withFixture(async (fixture) => {
    await writeLegacyStore(fixture);
    await runCli(fixture, ["import", "rust"]);

    const shown = await runCli(fixture, ["legacy", "show", featureId]);
    const selected = await runCli(fixture, [
      "legacy",
      "show",
      featureId,
      "--file",
      "design.md",
    ]);
    const missing = await runCli(fixture, ["legacy", "show", "feature-legacy-auror"]);

    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("title: Legacy Aurora");
    expect(shown.stdout).toContain("design.md");
    expect(shown.stdout).toContain("diagram.bin");
    expect(selected.exitCode).toBe(0);
    expect(selected.stdout).toContain("The quasar archive remains searchable.");
    expect(selected.stdout).not.toContain("diagram.bin");
    expect(missing.exitCode).not.toBe(0);
    expect(missing.stderr).toContain("LEGACY_NOT_FOUND");
    expect(missing.stderr).toContain(featureId);
  });
});

test("40 repeated rust imports replace legacy rows without changing counts", async () => {
  await withFixture(async (fixture) => {
    const source = await writeLegacyStore(fixture);

    const first = await runCli(fixture, ["import", "rust"]);
    const second = await runCli(fixture, ["import", "rust", "--path", source]);
    const database = targetDatabase(fixture);
    const counts = {
      cards: database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
        .get()?.count,
      decisions: database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_decisions")
        .get()?.count,
      files: database
        .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_files")
        .get()?.count,
    };
    database.close();

    expect(first.exitCode).toBe(0);
    expect(second.exitCode).toBe(0);
    expect(second.stdout).toBe(first.stdout);
    expect(counts).toEqual({ cards: 3, decisions: 2, files: 3 });
  });
});

test("41 legacy ids remain unknown to work, ready, and work list", async () => {
  await withFixture(async (fixture) => {
    await writeLegacyStore(fixture);
    const imported = await runCli(fixture, ["import", "rust"]);

    const started = await runCli(fixture, ["work", "start", featureId]);
    const done = await runCli(fixture, ["work", "done", featureId]);
    const ready = await runCli(fixture, ["ready"]);
    const listed = await runCli(fixture, ["work", "list"]);

    expect(imported.exitCode).toBe(0);
    expect(started.exitCode).not.toBe(0);
    expect(started.stderr).toContain("NOT_FOUND");
    expect(done.exitCode).not.toBe(0);
    expect(done.stderr).toContain("NOT_FOUND");
    expect(ready.stdout).not.toContain(featureId);
    expect(listed.stdout).not.toContain(featureId);
  });
});

test("E1/E2 ordinary verbs neither touch the Rust store nor auto-import legacy rows", async () => {
  await withFixture(async (fixture) => {
    const source = await writeLegacyStore(fixture);
    const before = sha256(await readFile(source));

    expect((await runCli(fixture, ["status"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["ready"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["search", "Aurora"])).exitCode).toBe(0);

    const database = targetDatabase(fixture);
    const count = database
      .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
      .get()?.count;
    database.close();
    expect(count).toBe(0);
    expect(sha256(await readFile(source))).toBe(before);
  });
});

test("52 legacy search summaries and result count stay bounded", async () => {
  await withFixture(async (fixture) => {
    await writeLegacyStore(fixture, 60);
    expect((await runCli(fixture, ["import", "rust"])).exitCode).toBe(0);

    const result = await runCli(fixture, ["search", "contextbomb"]);
    const lines = result.stdout.trim().split("\n");
    const hits = lines.filter((line) => line.startsWith("[legacy]"));

    expect(result.exitCode).toBe(0);
    expect(hits).toHaveLength(20);
    expect(lines.at(-1)).toBe("40 more — refine query");
    for (const hit of hits) {
      expect(hit).toMatch(/^\[legacy\] \S+ \(feature, closed\): Context bomb \d+ — /);
      expect(hit.split(" — ").at(-1)?.length).toBeLessThanOrEqual(200);
      expect(hit).not.toContain("schema_version:");
    }
  });
});
