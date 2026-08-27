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

interface SourceReceipt {
  artifact_type: string;
  card_id: string | null;
  created_at: string;
  id: string;
  payload_json: string;
}

interface SourceData {
  cards: SourceCard[];
  decisions: ImportedDecision[];
  files: ImportedFile[];
  receipts: SourceReceipt[];
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

function initializePromotionTables(context: PluginContext): void {
  context.store.migrate(`
    CREATE TABLE IF NOT EXISTS legacy_map (
      legacy_id TEXT PRIMARY KEY NOT NULL,
      native_id TEXT NOT NULL,
      entity_type TEXT NOT NULL CHECK(entity_type IN ('work', 'decision'))
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

function sourceData(path: string): SourceData {
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
    const receipts = hasTable(source, "receipt_artifacts")
      ? source
        .query<SourceReceipt, []>(
          `SELECT artifact_type, id, card_id, created_at, payload_json
             FROM receipt_artifacts ORDER BY artifact_type, id`,
        )
        .all()
      : [];
    return { cards, decisions: [...decisions.values()], files, receipts };
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
  data: SourceData,
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

const workKinds = new Set(["feature", "task", "idea", "bug", "progress"]);
const doneStatuses = new Set(["shipped", "closed", "verified"]);
const cancelledStatuses = new Set(["cancelled", "abandoned", "rejected", "dismissed"]);

function workKind(cardType: string): string {
  return cardType === "progress" ? "chore" : cardType;
}

function workState(status: string): "cancelled" | "done" | "open" {
  if (doneStatuses.has(status)) return "done";
  if (cancelledStatuses.has(status)) return "cancelled";
  return "open";
}

function decisionState(status: string): "draft" | "locked" | "superseded" {
  if (status === "open") return "draft";
  if (status === "superseded") return "superseded";
  return "locked";
}

function yamlReferences(yaml: string, key: string): string[] {
  const lines = yaml.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const match = line.match(new RegExp(`^(\\s*)${key}:\\s*(.*?)\\s*$`));
    if (!match) continue;
    const inline = match[2] ?? "";
    if (inline) {
      if (inline.startsWith("[") && inline.endsWith("]")) {
        return inline
          .slice(1, -1)
          .split(",")
          .map(scalar)
          .filter(Boolean);
      }
      return [scalar(inline)];
    }
    const fieldIndent = match[1]?.length ?? 0;
    const values: string[] = [];
    for (const candidate of lines.slice(index + 1)) {
      if (!candidate.trim()) continue;
      const item = candidate.match(/^(\s*)-\s+(.+?)\s*$/);
      if (item && (item[1]?.length ?? 0) >= fieldIndent) {
        values.push(scalar(item[2] ?? ""));
        continue;
      }
      const indent = candidate.match(/^\s*/)?.[0].length ?? 0;
      if (indent <= fieldIndent) break;
    }
    return values;
  }
  return [];
}

function payloadSummary(payload: string): string {
  let compact = payload;
  try {
    compact = JSON.stringify(JSON.parse(payload));
  } catch {}
  compact = compact.replaceAll(/\s+/g, " ").trim();
  return compact.length <= 240 ? compact : `${compact.slice(0, 239).trimEnd()}…`;
}

interface PromotionSummary {
  decisions: number;
  notes: number;
  receiptsSkipped: number;
  work: number;
}

function promotionAlreadyComplete(context: PluginContext, data: SourceData): boolean {
  const expected = data.cards.filter((card) =>
    workKinds.has(card.card_type) || card.card_type === "decision"
  );
  if (expected.length === 0) return false;
  const mapped = new Map(
    context.store.database
      .query<{ entity_type: string; legacy_id: string }, []>(
        "SELECT legacy_id, entity_type FROM legacy_map",
      )
      .all()
      .map((row) => [row.legacy_id, row.entity_type] as const),
  );
  return expected.every((card) =>
    mapped.get(card.id) === (card.card_type === "decision" ? "decision" : "work")
  );
}

function zeroPromotionSummary(data: SourceData): PromotionSummary {
  const workCardIds = new Set(
    data.cards.filter((card) => workKinds.has(card.card_type)).map((card) => card.id),
  );
  return {
    work: 0,
    decisions: 0,
    notes: 0,
    receiptsSkipped: data.receipts.filter((receipt) =>
      !receipt.card_id || !workCardIds.has(receipt.card_id)
    ).length,
  };
}

function promoteLegacyRows(context: PluginContext, data: SourceData): PromotionSummary {
  const database = context.store.database;
  const sessionId = context.sessions.current().id;
  const nativeIds = new Map(
    database
      .query<{ legacy_id: string; native_id: string }, []>(
        "SELECT legacy_id, native_id FROM legacy_map",
      )
      .all()
      .map((row) => [row.legacy_id, row.native_id] as const),
  );
  const workCards = data.cards.filter((card) => workKinds.has(card.card_type));
  const decisionCards = data.cards.filter((card) => card.card_type === "decision");
  const workCardIds = new Set(workCards.map((card) => card.id));
  let notes = 0;

  database.exec("BEGIN IMMEDIATE");
  try {
    const remaining = new Map(workCards.map((card) => [card.id, card] as const));
    while (remaining.size > 0) {
      let progressed = false;
      for (const card of [...remaining.values()]) {
        if (card.parent && workCardIds.has(card.parent) && !nativeIds.has(card.parent)) continue;
        const id = context.store.nextPrefixedId("work", "w");
        const state = workState(card.status);
        const parentId = card.parent ? nativeIds.get(card.parent) ?? null : null;
        const provenance = `imported from legacy card ${card.id}`;
        database
          .query(
            `INSERT INTO work
              (id, title, kind, state, parent_id, acceptance, atomic_reason, evidence,
               held_by, cancelled_at, cancel_reason, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?, ?, ?)`,
          )
          .run(
            id,
            card.title,
            workKind(card.card_type),
            state === "done" ? "done" : "open",
            parentId,
            state === "cancelled" ? card.updated_at : null,
            state === "cancelled" ? `imported legacy status: ${card.status}` : null,
            card.created_at,
            card.updated_at,
          );
        database
          .query("INSERT INTO legacy_map (legacy_id, native_id, entity_type) VALUES (?, ?, 'work')")
          .run(card.id, id);
        database
          .query("INSERT INTO work_notes (work_id, text, created_at) VALUES (?, ?, ?)")
          .run(id, provenance, card.updated_at);
        nativeIds.set(card.id, id);
        remaining.delete(card.id);
        notes += 1;
        context.log.append({
          type: "work.add",
          entityType: "work",
          entityId: id,
          sessionId,
          payload: { title: card.title, kind: workKind(card.card_type), parentId, importedFrom: card.id },
        });
        if (state === "done") {
          context.log.append({
            type: "work.done",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: { importedFrom: card.id, legacyStatus: card.status },
          });
        } else if (state === "cancelled") {
          context.log.append({
            type: "work.cancel",
            entityType: "work",
            entityId: id,
            sessionId,
            payload: { importedFrom: card.id, reason: `imported legacy status: ${card.status}` },
          });
        }
        context.log.append({
          type: "work.note",
          entityType: "work",
          entityId: id,
          sessionId,
          payload: { text: provenance },
        });
        progressed = true;
      }
      if (!progressed) {
        throw new CliError(
          "INVALID_LEGACY_STORE",
          `legacy work parent cycle: ${[...remaining.keys()].join(", ")}`,
        );
      }
    }

    let nextDecision = Number(context.store.nextPrefixedId("decisions", "d").slice(1));
    for (const card of decisionCards) {
      const id = `d${nextDecision}`;
      nextDecision += 1;
      nativeIds.set(card.id, id);
      database
        .query("INSERT INTO legacy_map (legacy_id, native_id, entity_type) VALUES (?, ?, 'decision')")
        .run(card.id, id);
    }

    const supersedes = new Map<string, string>();
    const supersededBy = new Map<string, string>();
    for (const card of decisionCards) {
      const predecessor = yamlReferences(card.card_yaml, "supersedes")[0];
      const successor = yamlReferences(card.card_yaml, "superseded_by")[0];
      if (predecessor && nativeIds.has(predecessor)) {
        supersedes.set(card.id, predecessor);
        supersededBy.set(predecessor, card.id);
      }
      if (successor && nativeIds.has(successor)) {
        supersededBy.set(card.id, successor);
        supersedes.set(successor, card.id);
      }
    }

    for (const card of decisionCards) {
      const id = nativeIds.get(card.id) as string;
      const state = decisionState(card.status);
      const provenance = `imported from legacy card ${card.id}`;
      const text = `${card.title} (${provenance})`;
      const workId = card.parent && workCardIds.has(card.parent)
        ? nativeIds.get(card.parent) ?? null
        : null;
      database
        .query(
          `INSERT INTO decisions
            (id, text, rationale, state, parent_id, work_id, supersedes_id,
             superseded_by_id, created_at, updated_at)
           VALUES (?, ?, ?, ?, NULL, ?, NULL, NULL, ?, ?)`,
        )
        .run(id, text, provenance, state, workId, card.created_at, card.updated_at);
      notes += 1;
      context.log.append({
        type: "decision.draft",
        entityType: "decision",
        entityId: id,
        sessionId,
        payload: { text, workId, importedFrom: card.id },
      });
      if (state !== "draft") {
        context.log.append({
          type: "decision.lock",
          entityType: "decision",
          entityId: id,
          sessionId,
          payload: { importedFrom: card.id, legacyStatus: card.status },
        });
      }
    }

    for (const card of decisionCards) {
      const id = nativeIds.get(card.id) as string;
      const predecessor = supersedes.get(card.id);
      const successor = supersededBy.get(card.id);
      database
        .query(
          `UPDATE decisions SET supersedes_id = ?, superseded_by_id = ? WHERE id = ?`,
        )
        .run(
          predecessor ? nativeIds.get(predecessor) ?? null : null,
          successor ? nativeIds.get(successor) ?? null : null,
          id,
        );
      if (predecessor) {
        context.log.append({
          type: "decision.supersede",
          entityType: "decision",
          entityId: id,
          sessionId,
          payload: { supersedesId: nativeIds.get(predecessor), importedFrom: card.id },
        });
      }
    }

    let receiptsSkipped = 0;
    for (const receipt of data.receipts) {
      const workId = receipt.card_id ? nativeIds.get(receipt.card_id) : null;
      if (!workId || !workCardIds.has(receipt.card_id as string)) {
        receiptsSkipped += 1;
        continue;
      }
      const note = `legacy receipt ${receipt.artifact_type} ${receipt.id}: ${payloadSummary(receipt.payload_json)}`;
      database
        .query("INSERT INTO work_notes (work_id, text, created_at) VALUES (?, ?, ?)")
        .run(workId, note, receipt.created_at);
      context.log.append({
        type: "work.note",
        entityType: "work",
        entityId: workId,
        sessionId,
        payload: { text: note },
      });
      notes += 1;
    }
    database.exec("COMMIT");
    return {
      work: workCards.length,
      decisions: decisionCards.length,
      notes,
      receiptsSkipped,
    };
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
  inject: ["work", "decision"],
  apply(context) {
    initializeLegacyTables(context);
    initializePromotionTables(context);
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
          const textFiles = data.files.filter((file) => file.text !== null).length;
          const result: CliResult = {
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
          if (invocation.options.promote === true && promotionAlreadyComplete(context, data)) {
            const promoted = zeroPromotionSummary(data);
            return {
              data: { ...(result.data as object), promoted },
              text:
                `promoted native: ${promoted.work} work created, ` +
                `${promoted.decisions} decisions created, ${promoted.notes} notes created, ` +
                `${promoted.receiptsSkipped} receipts skipped`,
            };
          }
          replaceLegacyRows(context, data);
          if (invocation.options.promote !== true) return result;
          const promoted = promoteLegacyRows(context, data);
          return {
            data: { ...(result.data as object), promoted },
            text:
              `${result.text}\npromoted native: ${promoted.work} work created, ` +
              `${promoted.decisions} decisions created, ${promoted.notes} notes created, ` +
              `${promoted.receiptsSkipped} receipts skipped`,
          };
        },
        {
          description: "Import a legacy Rust card store read-only.",
          flags: {
            "--path": { description: "Read this legacy store.sqlite file.", value: true },
            "--promote": { description: "Promote legacy cards into native work and decisions." },
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
