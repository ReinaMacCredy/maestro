import { Database } from "bun:sqlite";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { CliError, editDistance, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";

interface SourceCard {
  card_yaml: string;
  card_type: string;
  created_at: string;
  id: string;
  imported_at: string;
  parent: string | null;
  record_file: string;
  status: string;
  title: string;
  updated_at: string;
}

interface SourceFile {
  card_id: string;
  contents: Uint8Array | string;
  path: string;
  sha256: string;
}

interface ImportedFile {
  cardId: string;
  path: string;
  sha256: string;
  size: number;
  text: string | null;
}

interface ImportedDecision {
  cardId: string;
  id: string;
  sourcePath: string;
  status: string;
  title: string;
  yaml: string;
}

interface LegacyCardRow {
  card_yaml: string;
  id: string;
  title: string;
}

interface LegacyDecisionRow {
  decision_yaml: string;
  id: string;
  title: string;
}

interface LegacyFileRow {
  path: string;
  sha256: string;
  size: number;
  text_content: string | null;
}

const utf8 = new TextDecoder("utf-8", { fatal: true });

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function bytes(value: Uint8Array | string): Uint8Array {
  return typeof value === "string" ? new TextEncoder().encode(value) : value;
}

function text(value: Uint8Array): string | null {
  try {
    return utf8.decode(value);
  } catch {
    return null;
  }
}

function scalar(value: string): string {
  const trimmed = value.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    try {
      return JSON.parse(trimmed) as string;
    } catch {
      return trimmed.slice(1, -1);
    }
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
    return trimmed.slice(1, -1).replaceAll("''", "'");
  }
  return trimmed;
}

function decisionEntries(cardId: string, path: string, contents: string): ImportedDecision[] {
  const lines = contents.split(/\r?\n/);
  const markers = lines
    .map((line, index) => {
      const match = line.match(/^(\s*)-\s+/);
      return match ? { indent: match[1]?.length ?? 0, index } : null;
    })
    .filter((marker): marker is { indent: number; index: number } => marker !== null);
  if (markers.length === 0) return [];
  const entryIndent = Math.min(...markers.map((marker) => marker.indent));
  const starts = markers.filter((marker) => marker.indent === entryIndent).map(({ index }) => index);
  return starts.map((start, offset) => {
    const end = starts[offset + 1] ?? lines.length;
    const entry = lines.slice(start, end).join("\n").trimEnd();
    const fields = new Map<string, string>();
    for (const line of lines.slice(start, end)) {
      const match = line.match(/^\s*(?:-\s*)?(id|title|status):\s*(.+?)\s*$/);
      if (match?.[1] && match[2]) fields.set(match[1], scalar(match[2]));
    }
    const id = fields.get("id");
    if (!id) {
      throw new CliError(
        "INVALID_LEGACY_DECISIONS",
        `legacy decision entry in ${cardId}/${path} has no id`,
        { cardId, path },
      );
    }
    return {
      cardId,
      id,
      sourcePath: path,
      status: fields.get("status") ?? "unknown",
      title: fields.get("title") ?? id,
      yaml: entry,
    };
  });
}

function hasTable(database: Database, name: string): boolean {
  return database
    .query<{ present: number }, [string]>(
      "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get(name) !== null;
}

function initializeLegacyTables(context: PluginContext): void {
  context.store.migrate(`
    CREATE TABLE IF NOT EXISTS legacy_cards (
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
    CREATE TABLE IF NOT EXISTS legacy_files (
      card_id TEXT NOT NULL,
      path TEXT NOT NULL,
      sha256 TEXT NOT NULL,
      size INTEGER NOT NULL,
      text_content TEXT,
      PRIMARY KEY (card_id, path),
      FOREIGN KEY (card_id) REFERENCES legacy_cards(id) ON DELETE CASCADE
    );
    CREATE TABLE IF NOT EXISTS legacy_decisions (
      id TEXT PRIMARY KEY NOT NULL,
      card_id TEXT NOT NULL,
      title TEXT NOT NULL,
      status TEXT NOT NULL,
      source_path TEXT NOT NULL,
      decision_yaml TEXT NOT NULL
    );
  `);
}

function rebuildLegacySearch(context: PluginContext): void {
  if (!hasTable(context.store.database, "search_index")) return;
  context.store.database.run("DELETE FROM search_index WHERE surface = '[legacy]'");
  context.store.database.run(`
    INSERT INTO search_index(surface, entity_id, text)
    SELECT '[legacy]', id, title || ' ' || card_yaml FROM legacy_cards
  `);
  context.store.database.run(`
    INSERT INTO search_index(surface, entity_id, text)
    SELECT '[legacy]', card_id, path || ' ' || text_content
    FROM legacy_files WHERE text_content IS NOT NULL
  `);
  context.store.database.run(`
    INSERT INTO search_index(surface, entity_id, text)
    SELECT '[legacy]', card_id, title || ' ' || decision_yaml FROM legacy_decisions
  `);
}

function legacySearchNeedsRebuild(context: PluginContext): boolean {
  if (!hasTable(context.store.database, "search_index")) return false;
  const hasLegacyCards = context.store.database
    .query<{ present: number }, []>("SELECT 1 AS present FROM legacy_cards LIMIT 1")
    .get() !== null;
  if (!hasLegacyCards) return false;
  return context.store.database
    .query<{ present: number }, []>(
      "SELECT 1 AS present FROM search_index WHERE surface = '[legacy]' LIMIT 1",
    )
    .get() === null;
}

function sourceData(path: string): {
  cards: SourceCard[];
  decisions: ImportedDecision[];
  files: ImportedFile[];
} {
  if (!existsSync(path)) {
    throw new CliError("LEGACY_STORE_NOT_FOUND", `legacy Rust store not found: ${path}`, { path });
  }
  let source: Database;
  try {
    source = new Database(path, { readonly: true, strict: true });
  } catch (error) {
    throw new CliError(
      "INVALID_LEGACY_STORE",
      `cannot open legacy Rust store read-only: ${error instanceof Error ? error.message : String(error)}`,
      { path },
    );
  }
  try {
    if (!hasTable(source, "cards") || !hasTable(source, "card_files")) {
      throw new CliError(
        "INVALID_LEGACY_STORE",
        "legacy Rust store must contain cards and card_files tables",
        { path },
      );
    }
    const cards = source
      .query<SourceCard, []>(
        `SELECT id, card_type, parent, status, title, record_file, card_yaml,
                created_at, updated_at, imported_at
           FROM cards ORDER BY id`,
      )
      .all();
    const sourceFiles = source
      .query<SourceFile, []>(
        "SELECT card_id, path, contents, sha256 FROM card_files ORDER BY card_id, path",
      )
      .all();
    const decisions = new Map<string, ImportedDecision>();
    for (const card of cards) {
      if (card.card_type !== "decision") continue;
      decisions.set(card.id, {
        cardId: card.parent ?? card.id,
        id: card.id,
        sourcePath: card.record_file,
        status: card.status,
        title: card.title,
        yaml: card.card_yaml,
      });
    }
    const files = sourceFiles.map((file): ImportedFile => {
      const contents = bytes(file.contents);
      const decoded = text(contents);
      if (decoded !== null && (file.path === "decisions.yaml" || file.path.endsWith("/decisions.yaml"))) {
        for (const decision of decisionEntries(file.card_id, file.path, decoded)) {
          if (!decisions.has(decision.id)) decisions.set(decision.id, decision);
        }
      }
      return {
        cardId: file.card_id,
        path: file.path,
        sha256: file.sha256,
        size: contents.byteLength,
        text: decoded,
      };
    });
    return { cards, decisions: [...decisions.values()], files };
  } catch (error) {
    if (error instanceof CliError) throw error;
    throw new CliError(
      "INVALID_LEGACY_STORE",
      `cannot read legacy Rust store: ${error instanceof Error ? error.message : String(error)}`,
      { path },
    );
  } finally {
    source.close();
  }
}

function replaceLegacyRows(
  context: PluginContext,
  data: ReturnType<typeof sourceData>,
): void {
  const database = context.store.database;
  database.exec("BEGIN IMMEDIATE");
  try {
    if (hasTable(database, "search_index")) {
      database.run("DELETE FROM search_index WHERE surface = '[legacy]'");
    }
    database.run("DELETE FROM legacy_decisions");
    database.run("DELETE FROM legacy_files");
    database.run("DELETE FROM legacy_cards");
    const insertCard = database.query(
      `INSERT INTO legacy_cards
        (id, card_type, parent, status, title, record_file, card_yaml, created_at, updated_at, imported_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    );
    for (const card of data.cards) {
      insertCard.run(
        card.id,
        card.card_type,
        card.parent,
        card.status,
        card.title,
        card.record_file,
        card.card_yaml,
        card.created_at,
        card.updated_at,
        card.imported_at,
      );
    }
    const insertFile = database.query(
      `INSERT INTO legacy_files (card_id, path, sha256, size, text_content)
       VALUES (?, ?, ?, ?, ?)`,
    );
    for (const file of data.files) {
      insertFile.run(file.cardId, file.path, file.sha256, file.size, file.text);
    }
    const insertDecision = database.query(
      `INSERT INTO legacy_decisions (id, card_id, title, status, source_path, decision_yaml)
       VALUES (?, ?, ?, ?, ?, ?)`,
    );
    for (const decision of data.decisions) {
      insertDecision.run(
        decision.id,
        decision.cardId,
        decision.title,
        decision.status,
        decision.sourcePath,
        decision.yaml,
      );
    }
    rebuildLegacySearch(context);
    database.exec("COMMIT");
  } catch (error) {
    database.exec("ROLLBACK");
    throw error;
  }
}

function suggestions(context: PluginContext, id: string): string[] {
  return context.store.database
    .query<{ id: string }, []>("SELECT id FROM legacy_cards ORDER BY id")
    .all()
    .map((row) => ({ id: row.id, distance: editDistance(id, row.id) }))
    .sort((left, right) => left.distance - right.distance || left.id.localeCompare(right.id))
    .slice(0, 3)
    .map(({ id: candidate }) => candidate);
}

function formatFile(file: LegacyFileRow): string {
  const metadata = `${file.path} sha256=${file.sha256} size=${file.size}`;
  return file.text_content === null ? metadata : `${metadata}\n${file.text_content.trimEnd()}`;
}

function showLegacy(context: PluginContext, invocation: CliInvocation): CliResult {
  const id = required(invocation, 0, "legacy card id");
  const card = context.store.database
    .query<LegacyCardRow, [string]>(
      "SELECT id, title, card_yaml FROM legacy_cards WHERE id = ?",
    )
    .get(id);
  if (!card) {
    const near = suggestions(context, id);
    throw new CliError(
      "LEGACY_NOT_FOUND",
      `legacy card not found: ${id}${near.length > 0 ? `; nearest: ${near.join(", ")}` : ""}`,
      { id, suggestions: near },
    );
  }
  const selected = invocation.options.file;
  if (typeof selected === "string") {
    const file = context.store.database
      .query<LegacyFileRow, [string, string]>(
        `SELECT path, sha256, size, text_content FROM legacy_files
          WHERE card_id = ? AND path = ?`,
      )
      .get(id, selected);
    if (!file) {
      const paths = context.store.database
        .query<{ path: string }, [string]>(
          "SELECT path FROM legacy_files WHERE card_id = ? ORDER BY path",
        )
        .all(id)
        .map((row) => row.path);
      throw new CliError(
        "LEGACY_FILE_NOT_FOUND",
        `legacy file not found: ${id}/${selected}; available: ${paths.join(", ") || "none"}`,
        { id, path: selected, suggestions: paths },
      );
    }
    return { data: { card: { id: card.id, title: card.title }, file }, text: formatFile(file) };
  }
  const files = context.store.database
    .query<LegacyFileRow, [string]>(
      "SELECT path, sha256, size, text_content FROM legacy_files WHERE card_id = ? ORDER BY path",
    )
    .all(id);
  const decisions = context.store.database
    .query<LegacyDecisionRow, [string, string]>(
      `SELECT id, title, decision_yaml FROM legacy_decisions
        WHERE card_id = ? OR id = ? ORDER BY id`,
    )
    .all(id, id);
  const sections = [
    `[legacy] ${card.id}`,
    card.card_yaml.trimEnd(),
    `files:\n${files.map(formatFile).join("\n") || "none"}`,
  ];
  if (decisions.length > 0) {
    sections.push(
      `decisions:\n${decisions.map((decision) => decision.decision_yaml.trimEnd()).join("\n")}`,
    );
  }
  return { data: { card, files, decisions }, text: sections.join("\n") };
}

export const importRustPlugin: BuiltInPlugin = {
  name: "import-rust",
  apply(context) {
    initializeLegacyTables(context);
    if (!context.store.readOnly && legacySearchNeedsRebuild(context)) rebuildLegacySearch(context);
    context.effect(() =>
      context.cli.register(
        "import rust",
        (invocation): CliResult => {
          const override = invocation.options.path;
          const path = typeof override === "string"
            ? override
            : join(dirname(context.store.path), "store.sqlite");
          const data = sourceData(path);
          replaceLegacyRows(context, data);
          const textFiles = data.files.filter((file) => file.text !== null).length;
          return {
            data: {
              path,
              cards: data.cards.length,
              files: data.files.length,
              textFiles,
              decisions: data.decisions.length,
            },
            text: [
              `imported legacy: ${data.cards.length} cards, ${data.files.length} files (${textFiles} text), ${data.decisions.length} decisions`,
              data.cards[0]
                ? `read them: maestro legacy show ${data.cards[0].id}`
                : null,
            ].filter((line): line is string => line !== null).join("\n"),
          };
        },
        {
          description: "Import a legacy Rust card store read-only.",
          flags: {
            "--path": { description: "Read this legacy store.sqlite file.", value: true },
          },
          rootDescription: "Import one-shot data from legacy Maestro stores.",
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "legacy show",
        (invocation): CliResult => showLegacy(context, invocation),
        {
          description: "Show one read-only legacy card and its files.",
          flags: {
            "--file": { description: "Show only this legacy file path.", value: true },
          },
          mutates: false,
          positionals: [{ name: "id", required: true }],
          rootDescription: "Read imported legacy cards without changing them.",
        },
      ),
    );
  },
};
