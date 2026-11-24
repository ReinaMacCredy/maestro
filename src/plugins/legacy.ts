import { CliError, editDistance, requiredPosition, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { tableExists } from "../kernel/store.ts";

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

function initializeLegacyTables(context: PluginContext): void {
  context.store.migrate(`
    CREATE TABLE IF NOT EXISTS legacy_cards (
      id TEXT PRIMARY KEY NOT NULL,
      source TEXT NOT NULL DEFAULT 'store',
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
      source TEXT NOT NULL DEFAULT 'store',
      path TEXT NOT NULL,
      sha256 TEXT NOT NULL,
      size INTEGER NOT NULL,
      text_content TEXT,
      PRIMARY KEY (card_id, path),
      FOREIGN KEY (card_id) REFERENCES legacy_cards(id) ON DELETE CASCADE
    );
    CREATE TABLE IF NOT EXISTS legacy_decisions (
      id TEXT PRIMARY KEY NOT NULL,
      source TEXT NOT NULL DEFAULT 'store',
      card_id TEXT NOT NULL,
      title TEXT NOT NULL,
      status TEXT NOT NULL,
      source_path TEXT NOT NULL,
      decision_yaml TEXT NOT NULL
    );
  `);
}

function rebuildLegacySearch(context: PluginContext): void {
  if (!tableExists(context.store.database, "search_index")) return;
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
  if (!tableExists(context.store.database, "search_index")) return false;
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
  const id = requiredPosition(invocation, 0, "legacy card id");
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

export const legacyPlugin: BuiltInPlugin = {
  name: "legacy",
  apply(context) {
    initializeLegacyTables(context);
    if (!context.store.readOnly && legacySearchNeedsRebuild(context)) rebuildLegacySearch(context);
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
