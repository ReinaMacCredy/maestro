import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, setPlugin, withFixture, type Fixture } from "./helpers.ts";

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

async function writePromotionStore(fixture: Fixture): Promise<string> {
  const path = await writeLegacyStore(fixture);
  const database = new Database(path, { strict: true });
  const now = "2026-08-27T00:00:00Z";
  const insertCard = database.query(
    `INSERT INTO cards
      (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
     VALUES (?, ?, ?, ?, ?, 'card.yaml', ?, ?, ?, ?)`,
  );
  for (const card of [
    {
      id: "task-legacy-child",
      type: "task",
      parent: featureId,
      status: "draft",
      title: "Legacy child task",
      yaml: `id: task-legacy-child\ntype: task\nparent: ${featureId}\nstatus: draft\ntitle: Legacy child task\n`,
    },
    {
      id: "idea-legacy-dismissed",
      type: "idea",
      parent: null,
      status: "dismissed",
      title: "Legacy dismissed idea",
      yaml: "id: idea-legacy-dismissed\ntype: idea\nstatus: dismissed\ntitle: Legacy dismissed idea\n",
    },
    {
      id: "bug-legacy-closed",
      type: "bug",
      parent: null,
      status: "closed",
      title: "Legacy closed bug",
      yaml: "id: bug-legacy-closed\ntype: bug\nstatus: closed\ntitle: Legacy closed bug\n",
    },
    {
      id: "progress-legacy-running",
      type: "progress",
      parent: null,
      status: "in_progress",
      title: "Legacy running progress",
      yaml: "id: progress-legacy-running\ntype: progress\nstatus: in_progress\ntitle: Legacy running progress\n",
    },
    {
      id: "dec-legacy-old",
      type: "decision",
      parent: featureId,
      status: "superseded",
      title: "Legacy old ruling",
      yaml: "id: dec-legacy-old\ntype: decision\nstatus: superseded\nparent: feature-legacy-aurora\nextra:\n  superseded_by: dec-legacy-new\n",
    },
    {
      id: "dec-legacy-new",
      type: "decision",
      parent: featureId,
      status: "locked",
      title: "Legacy replacement ruling",
      yaml: "id: dec-legacy-new\ntype: decision\nstatus: locked\nparent: feature-legacy-aurora\nextra:\n  decision: Use corrected replacement ruling\n  supersedes:\n  - dec-legacy-old\n",
    },
    {
      id: "dec-legacy-open",
      type: "decision",
      parent: "missing-parent",
      status: "open",
      title: "Legacy open ruling",
      yaml: "id: dec-legacy-open\ntype: decision\nstatus: open\nparent: missing-parent\n",
    },
  ]) {
    insertCard.run(
      card.id,
      card.type,
      card.parent,
      card.status,
      card.title,
      card.yaml,
      now,
      now,
      now,
    );
  }
  database.exec(`
    CREATE TABLE receipt_artifacts (
      artifact_type TEXT NOT NULL,
      id TEXT NOT NULL,
      card_id TEXT,
      created_at TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      PRIMARY KEY (artifact_type, id)
    );
  `);
  const insertReceipt = database.query(
    `INSERT INTO receipt_artifacts (artifact_type, id, card_id, created_at, payload_json)
     VALUES (?, ?, ?, ?, ?)`,
  );
  insertReceipt.run(
    "verification",
    "receipt-legacy-aurora",
    featureId,
    now,
    JSON.stringify({
      result: "pass",
      checks: 3,
      details: "proof ".repeat(80),
      exactWhitespace: "two  spaces\nand a newline",
      tail: "preserved",
    }),
  );
  insertReceipt.run(
    "verification",
    "receipt-orphan",
    "missing-card",
    now,
    JSON.stringify({ result: "orphan" }),
  );
  database.close();
  return path;
}

async function writeArchiveStore(fixture: Fixture): Promise<string> {
  const path = join(fixture.repo, "archive-cards.sqlite");
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
  const archiveText = "# Archived Nebula\narchivequasar remains searchable\n";
  const archivePath = "notes.md";
  const pathBytes = new TextEncoder().encode(archivePath);
  const contentBytes = new TextEncoder().encode(archiveText);
  const magic = new TextEncoder().encode("MAESTRO_ARCHIVE_SNAPSHOT_V1\n");
  const snapshot = new Uint8Array(magic.length + 4 + 4 + 8 + pathBytes.length + contentBytes.length);
  snapshot.set(magic);
  const view = new DataView(snapshot.buffer);
  let offset = magic.length;
  view.setUint32(offset, 1, true);
  offset += 4;
  view.setUint32(offset, pathBytes.length, true);
  offset += 4;
  view.setBigUint64(offset, BigInt(contentBytes.length), true);
  offset += 8;
  snapshot.set(pathBytes, offset);
  offset += pathBytes.length;
  snapshot.set(contentBytes, offset);
  const compressed = Bun.zstdCompressSync(snapshot);
  const insertSnapshot = database.query(
    `INSERT INTO archived_snapshots
      (id, archived_at, source_relpath, manifest_json, snapshot_zstd,
       snapshot_sha256, search_text, last_checked_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, NULL)`,
  );
  insertSnapshot.run(
    "snapshot-legacy-nebula",
    "2026-06-30T00:00:00Z",
    "archive/snapshot-legacy-nebula",
    JSON.stringify({
      format_version: "maestro.archive.snapshot.v1",
      files: [{ path: archivePath, size: contentBytes.length, sha256: sha256(contentBytes) }],
    }),
    compressed,
    sha256(compressed),
    "title: Archived Nebula\ndescription: archivequasar remains searchable\n",
  );
  const corrupt = new Uint8Array([0x00, 0x01, 0x02]);
  insertSnapshot.run(
    "snapshot-legacy-fallback",
    "2026-06-30T00:01:00Z",
    "archive/snapshot-legacy-fallback",
    JSON.stringify({ format_version: "maestro.archive.snapshot.v1", files: [] }),
    corrupt,
    sha256(corrupt),
    "title: Archived Fallback\ndescription: fallbackneedle remains searchable\n",
  );
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

    expect(title.stdout).toContain(`${idFrom(native)} (task, open): Native Aurora follow-up`);
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
    const ready = await runCli(fixture, ["ready", "--json"]);
    expect(ready.exitCode).toBe(0);
    const readyData = JSON.parse(ready.stdout).data as {
      gated: Array<{ id: string }>;
      works: Array<{ id: string }>;
    };
    const listed = await runCli(fixture, ["work", "list"]);

    expect(imported.exitCode).toBe(0);
    expect(started.exitCode).not.toBe(0);
    expect(started.stderr).toContain("NOT_FOUND");
    expect(done.exitCode).not.toBe(0);
    expect(done.stderr).toContain("NOT_FOUND");
    expect(readyData.works.map((work) => work.id)).not.toContain(featureId);
    expect(readyData.gated.map((work) => work.id)).not.toContain(featureId);
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
    expect(hits).toHaveLength(5);
    expect(lines.at(-1)).toBe("55 more; raise --limit to see them");
    for (const hit of hits) {
      expect(hit).toMatch(/^\[legacy\] \S+ \(feature, closed\): Context bomb \d+ — /);
      expect(hit.split(" — ").at(-1)?.length).toBeLessThanOrEqual(200);
      expect(hit).not.toContain("schema_version:");
    }
  });
});

test("300 --promote maps legacy cards, decisions, receipts, provenance, and events", async () => {
  await withFixture(async (fixture) => {
    const source = await writePromotionStore(fixture);

    const promoted = await runCli(fixture, ["import", "rust", "--path", source, "--promote"]);

    expect(promoted.exitCode).toBe(0);
    expect(promoted.stdout).toContain("5 work created");
    expect(promoted.stdout).toContain("5 decisions created");
    expect(promoted.stdout).toContain("11 notes created");
    expect(promoted.stdout).toContain("1 receipts skipped");

    const database = targetDatabase(fixture);
    const mappings = database
      .query<{ entity_type: string; legacy_id: string; native_id: string }, []>(
        "SELECT legacy_id, native_id, entity_type FROM legacy_map ORDER BY legacy_id",
      )
      .all();
    const nativeId = (legacyId: string): string => {
      const id = mappings.find((mapping) => mapping.legacy_id === legacyId)?.native_id;
      if (!id) throw new Error(`missing native mapping for ${legacyId}`);
      return id;
    };
    const featureWork = nativeId(featureId);
    const taskWork = nativeId("task-legacy-child");
    const oldDecision = nativeId("dec-legacy-old");
    const newDecision = nativeId("dec-legacy-new");
    expect(mappings).toHaveLength(10);
    expect(database.query("SELECT kind, state, held_by FROM work WHERE id = ?").get(featureWork))
      .toEqual({ kind: "feature", state: "done", held_by: null });
    expect(database.query("SELECT kind, state, parent_id, held_by FROM work WHERE id = ?").get(taskWork))
      .toEqual({ kind: "task", state: "open", parent_id: featureWork, held_by: null });
    expect(database.query("SELECT kind, state, cancelled_at FROM work WHERE id = ?").get(nativeId("idea-legacy-dismissed")))
      .toEqual({ kind: "idea", state: "open", cancelled_at: expect.any(String) });
    expect(database.query("SELECT kind, state FROM work WHERE id = ?").get(nativeId("bug-legacy-closed")))
      .toEqual({ kind: "bug", state: "done" });
    expect(database.query("SELECT kind, state, held_by FROM work WHERE id = ?").get(nativeId("progress-legacy-running")))
      .toEqual({ kind: "chore", state: "open", held_by: null });
    expect(
      database.query("SELECT state, work_id, superseded_by_id FROM decisions WHERE id = ?").get(oldDecision),
    ).toEqual({ state: "superseded", work_id: featureWork, superseded_by_id: newDecision });
    expect(
      database.query("SELECT state, work_id, supersedes_id FROM decisions WHERE id = ?").get(newDecision),
    ).toEqual({ state: "locked", work_id: featureWork, supersedes_id: oldDecision });
    expect(
      database.query("SELECT state, work_id FROM decisions WHERE id = ?").get(nativeId("dec-legacy-open")),
    ).toEqual({ state: "draft", work_id: null });

    const workNotes = database
      .query<{ text: string }, [string]>("SELECT text FROM work_notes WHERE work_id = ? ORDER BY id")
      .all(featureWork)
      .map((row) => row.text);
    expect(workNotes).toContain(`imported from legacy card ${featureId}`);
    const receiptNote = workNotes.find((note) => note.startsWith("legacy receipt verification"));
    expect(receiptNote).toBeString();
    const receiptPayload = JSON.parse(receiptNote?.slice(receiptNote.indexOf(": ") + 2) ?? "null") as {
      exactWhitespace?: string;
      tail?: string;
    };
    expect(receiptPayload.exactWhitespace).toBe("two  spaces\nand a newline");
    expect(receiptPayload.tail).toBe("preserved");
    expect(
      database.query<{ rationale: string; text: string }, [string]>(
        "SELECT text, rationale FROM decisions WHERE id = ?",
      ).get(newDecision),
    ).toEqual({
      text: expect.stringContaining("Use corrected replacement ruling"),
      rationale: "imported from legacy card dec-legacy-new",
    });
    expect(
      database
        .query<{ type: string }, [string]>(
          "SELECT type FROM event_log WHERE entity_type = 'decision' AND entity_id = ? ORDER BY id",
        )
        .all(newDecision)
        .map((event) => event.type),
    ).toEqual(["decision.draft", "decision.supersede", "decision.lock"]);
    expect(
      database
        .query<{ entity_id: string; type: string }, [string, string]>(
          `SELECT entity_id, type FROM event_log
           WHERE entity_type = 'decision' AND entity_id IN (?, ?) ORDER BY id`,
        )
        .all(oldDecision, newDecision),
    ).toEqual([
      { entity_id: oldDecision, type: "decision.draft" },
      { entity_id: oldDecision, type: "decision.lock" },
      { entity_id: newDecision, type: "decision.draft" },
      { entity_id: newDecision, type: "decision.supersede" },
      { entity_id: newDecision, type: "decision.lock" },
    ]);
    expect(
      database.query<{ count: number }, []>(
        "SELECT count(*) AS count FROM event_log WHERE entity_type IN ('work', 'decision')",
      ).get()?.count,
    ).toBeGreaterThanOrEqual(10);
    database.close();

    const shownWork = await runCli(fixture, ["work", "show", featureWork]);
    const decisions = await runCli(fixture, ["decision", "list"]);
    expect(shownWork.stdout).toContain(`note: imported from legacy card ${featureId}`);
    expect(decisions.stdout).toContain("imported from legacy card dec-legacy-new");
  });
});

test("301 a repeated --promote run creates nothing and preserves native rows and events", async () => {
  await withFixture(async (fixture) => {
    const source = await writePromotionStore(fixture);
    const first = await runCli(fixture, ["import", "rust", "--path", source, "--promote"]);
    expect(first.exitCode).toBe(0);

    const beforeDatabase = targetDatabase(fixture);
    const before = beforeDatabase
      .query<{ decisions: number; events: number; mappings: number; notes: number; work: number }, []>(
        `SELECT
          (SELECT count(*) FROM work) AS work,
          (SELECT count(*) FROM decisions) AS decisions,
          (SELECT count(*) FROM work_notes) AS notes,
          (SELECT count(*) FROM event_log) AS events,
          (SELECT count(*) FROM legacy_map) AS mappings`,
      )
      .get();
    beforeDatabase.close();

    const second = await runCli(fixture, ["import", "rust", "--path", source, "--promote"]);

    expect(second.exitCode).toBe(0);
    expect(second.stdout).toContain("0 work created");
    expect(second.stdout).toContain("0 decisions created");
    expect(second.stdout).toContain("0 notes created");
    expect(second.stdout).toContain("1 receipts skipped");
    const afterDatabase = targetDatabase(fixture);
    const after = afterDatabase
      .query<{ decisions: number; events: number; mappings: number; notes: number; work: number }, []>(
        `SELECT
          (SELECT count(*) FROM work) AS work,
          (SELECT count(*) FROM decisions) AS decisions,
          (SELECT count(*) FROM work_notes) AS notes,
          (SELECT count(*) FROM event_log) AS events,
          (SELECT count(*) FROM legacy_map) AS mappings`,
      )
      .get();
    afterDatabase.close();
    expect(after).toEqual(before);
  });
});

test("302 archive-only Rust stores decode snapshots into searchable legacy files and report skips", async () => {
  await withFixture(async (fixture) => {
    const source = await writeArchiveStore(fixture);
    const before = sha256(await readFile(source));

    const imported = await runCli(fixture, ["import", "rust", "--path", source]);
    const searched = await runCli(fixture, ["search", "archivequasar"]);

    expect(imported.exitCode).toBe(0);
    expect(imported.stdout).toContain("2 archived snapshots");
    expect(imported.stdout).toContain("1 compressed payloads skipped");
    expect(searched.exitCode).toBe(0);
    expect(searched.stdout).toContain("[legacy] snapshot-legacy-nebula");
    const fallback = await runCli(fixture, ["search", "fallbackneedle"]);
    expect(fallback.exitCode).toBe(0);
    expect(fallback.stdout).toContain("[legacy] snapshot-legacy-fallback");
    expect(sha256(await readFile(source))).toBe(before);
    const database = targetDatabase(fixture);
    expect(
      database
        .query("SELECT card_id, path, text_content FROM legacy_files WHERE card_id = ?")
        .get("snapshot-legacy-nebula"),
    ).toEqual({
      card_id: "snapshot-legacy-nebula",
      path: "archive/snapshot-legacy-nebula/notes.md",
      text_content: "# Archived Nebula\narchivequasar remains searchable\n",
    });
    expect(
      database
        .query("SELECT path, text_content FROM legacy_files WHERE card_id = ?")
        .get("snapshot-legacy-fallback"),
    ).toEqual({
      path: "archive/snapshot-legacy-fallback",
      text_content: "title: Archived Fallback\ndescription: fallbackneedle remains searchable\n",
    });
    database.close();
  });
});

test("303 reference import stays available when native work promotion is disabled", async () => {
  await withFixture(async (fixture) => {
    await writeLegacyStore(fixture);
    await setPlugin(fixture, "work", true);

    const imported = await runCli(fixture, ["import", "rust"]);
    const shown = await runCli(fixture, ["legacy", "show", featureId]);

    expect(imported.exitCode).toBe(0);
    expect(imported.stdout).toBe(
      `imported legacy: 3 cards, 3 files (2 text), 2 decisions\n` +
        `read them: maestro legacy show dec-aurora-color\n`,
    );
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("title: Legacy Aurora");
  });
});

test("304 --promote fails before reference rows when native promotion is disabled", async () => {
  await withFixture(async (fixture) => {
    const source = await writeLegacyStore(fixture);
    await setPlugin(fixture, "work", true);

    const promoted = await runCli(fixture, ["import", "rust", "--path", source, "--promote"]);

    expect(promoted.exitCode).toBe(1);
    const error = JSON.parse(promoted.stderr) as { error: { code: string; message: string } };
    expect(error.error).toEqual({
      code: "PROMOTION_UNAVAILABLE",
      message: "--promote requires the native work and decision plugins",
    });
    const database = targetDatabase(fixture);
    expect(database.query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards").get()?.count)
      .toBe(0);
    database.close();
  });
});
