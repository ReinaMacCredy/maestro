import { Database } from "bun:sqlite";
import { existsSync, mkdirSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOptions,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { resolveStoreLocation, tableExists } from "../kernel/store.ts";
import type {
  DispatchService,
  HandbackRecord,
  HandbackService,
} from "./dispatch.ts";
import { formatImportReport, importWaymarkTree } from "./bundle-import.ts";
import { resolveHubRoom, samePath } from "./home.ts";
import { registerSessionCommand } from "./session-required.ts";
import { latestCheckpoints, type WorkService } from "./work.ts";

export interface BundleRecord {
  id: string;
  state: "active" | "archived";
  pausedAt: string | null;
  directory: string;
  spec: string | null;
  notes: string | null;
  verify: string | null;
  createdAt: string;
  updatedAt: string;
}

interface BundleRow {
  id: string;
  state: "active" | "archived";
  paused_at: string | null;
  directory: string;
  spec: string | null;
  notes: string | null;
  verify: string | null;
  created_at: string;
  updated_at: string;
}

const trioFiles = ["SPEC.md", "NOTES.md", "VERIFY.md"] as const;
const handoffPlaceholder = "<!-- handoff: unfilled -->";
const handoffSectionNames = [
  "Current State",
  "Next Action",
  "Authority",
  "Failed approaches",
  "Do not repeat",
] as const;
type HandoffSectionName = typeof handoffSectionNames[number];

function specTemplate(id: string): string {
  return `# SPEC — ${id}

## Problem

## Solution

## Scope

## Anti-goals

Each anti-goal gets a matching VERIFY.md check.

## Decisions

Ids only: record via \`maestro decision draft "<text>" --rationale "<why>"\`, lock, and list the ids here; a decision taken in the Hub store is listed as \`hub:<id>\`. \`maestro bundle show ${id}\` renders each with its ruling and rejected alternative.

## Units

One unit is one commit, or one PR under a grant, with its own evidence. Write one \`### U<n>: <title>\` block per unit in build order. \`Depends on\` names the units it waits for, or none. \`Files\` lists each path as edit, create or delete. \`Build\` is the one change, naming the symbol and the file. \`You will see\` is the observable result once it lands, the exact output line or screen state. Each unit gets a VERIFY.md row whose Scenario names its id. Delete this section when the change is one or two files with an obvious approach. \`maestro bundle check ${id}\` lints this section and VERIFY.md.

### U1: <title>

Depends on:
Files:
Build:
You will see:

## Derived artifacts

A table, map, graph or measurement any decision above rests on ships in this bundle beside SPEC.md, named here with the command or script that produced it and what that method over- or under-approximates. The conclusion is not the evidence. Leave empty when no decision here rests on a derivation.

## Red tests

Apply Testing discipline in \`~/maestro/WORKFLOW.md#testing-discipline\`. List necessary new tests and the gaps they cover; reference existing checks in VERIFY.md. Leave empty when existing evidence suffices. This is not a quota or an exhaustive whitelist.
`;
}

// git already knows the base; an empty Base line is the first thing a
// successor finds missing when a session dies before writing its handoff.
async function baseLine(root: string): Promise<string> {
  const read = async (args: string[]): Promise<string | null> => {
    const child = Bun.spawn(["git", ...args], {
      cwd: root,
      stdout: "pipe",
      stderr: "ignore",
    });
    const [text, code] = await Promise.all([new Response(child.stdout).text(), child.exited]);
    const value = text.trim();
    return code === 0 && value ? value : null;
  };
  const commit = await read(["rev-parse", "--short", "HEAD"]);
  if (!commit) return "Base:";
  const branch = await read(["branch", "--show-current"]);
  return branch ? `Base: ${commit} (${branch})` : `Base: ${commit}`;
}

function notesTemplate(id: string, base: string): string {
  return `# NOTES — ${id}

Overwritten handoff, never appended; history lives in trace and decisions.

${base}

## Current State

## Next Action

## Authority

Transferred:

Retained:

## Failed approaches

## Do not repeat
`;
}

function verifyTemplate(id: string): string {
  return `# VERIFY — ${id}

One row per SPEC unit, its Scenario naming the unit id, and one row per anti-goal. A unit, live or perf cell is filled only with evidence, or \`n/a: <reason>\`, and the Commit cell stamps the row. \`maestro bundle check ${id} --final\` lints the filled table.

| # | Scenario | unit | live | perf | Commit |
|---|----------|------|------|------|--------|
| 1 | U1: <scenario> | | | | |
`;
}

const unitLabels = ["Depends on", "Files", "Build", "You will see"] as const;
const requiredUnitLabels = ["Depends on", "Files", "You will see"] as const;
const verifyColumns = ["Scenario", "unit", "live", "perf", "Commit"] as const;
const stampedColumns = ["unit", "live", "perf", "Commit"] as const;

interface PlanLine {
  n: number;
  text: string;
}

function proseLines(text: string): PlanLine[] {
  let fenced = false;
  return text.split("\n").flatMap((line, index) => {
    if (/^ {0,3}(```|~~~)/.test(line)) {
      fenced = !fenced;
      return [];
    }
    return fenced ? [] : [{ n: index + 1, text: line }];
  });
}

function tableCells(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/(?<!\\)\|$/, "")
    .split(/(?<!\\)\|/)
    .map((cell) => cell.trim());
}

function checkPlan(
  spec: string,
  verify: string,
  specPath: string,
  verifyPath: string,
  final: boolean,
): { problems: string[]; rows: number; units: number } | null {
  const specLines = proseLines(spec);
  const start = specLines.findIndex((line) => line.text.trim() === "## Units");
  const unitsHeading = specLines[start];
  if (!unitsHeading) return null;
  const problems: string[] = [];
  const after = specLines.slice(start + 1);
  const end = after.findIndex((line) => /^#{1,2} /.test(line.text));
  const section = end === -1 ? after : after.slice(0, end);
  const headings = section.flatMap((line, index) => (line.text.startsWith("### ") ? [index] : []));
  if (headings.length === 0) problems.push(`${specPath}:${unitsHeading.n} ## Units is missing a ### unit`);
  const unitIds: string[] = [];
  headings.forEach((headingIndex, order) => {
    const heading = section[headingIndex] as PlanLine;
    const id = heading.text.match(/^### (U\d+)\b/)?.[1];
    if (!id) {
      problems.push(`${specPath}:${heading.n} unit heading is missing a U<n> id`);
      return;
    }
    unitIds.push(id);
    const body = section.slice(headingIndex + 1, headings[order + 1] ?? section.length);
    const labelled = body.flatMap((line, index) => {
      const label = unitLabels.find((name) => line.text.startsWith(`${name}:`));
      return label ? [{ index, label, line }] : [];
    });
    for (const label of requiredUnitLabels) {
      const at = labelled.findIndex((entry) => entry.label === label);
      const entry = labelled[at];
      if (!entry) {
        problems.push(`${specPath}:${heading.n} ${id} is missing "${label}:"`);
        continue;
      }
      const value = [
        entry.line.text.slice(label.length + 1),
        ...body.slice(entry.index + 1, labelled[at + 1]?.index ?? body.length).map((line) => line.text),
      ].join("\n");
      if (value.trim() === "") problems.push(`${specPath}:${entry.line.n} ${id} is missing "${label}:"`);
    }
  });

  const verifyLines = proseLines(verify);
  const headerAt = verifyLines.findIndex((line) => line.text.trim().startsWith("|"));
  const header = verifyLines[headerAt];
  const names = header ? tableCells(header.text).map((cell) => cell.toLowerCase()) : [];
  const column = new Map(verifyColumns.map((name) => [name, names.indexOf(name.toLowerCase())]));
  const rows: PlanLine[] = [];
  if (!header) {
    problems.push(`${verifyPath}:1 table with ${verifyColumns.join(", ")} columns is missing`);
  } else {
    const missing = verifyColumns.filter((name) => column.get(name) === -1);
    if (missing.length > 0) problems.push(`${verifyPath}:${header.n} table is missing the ${missing.join(", ")} column${missing.length === 1 ? "" : "s"}`);
    for (const line of verifyLines.slice(headerAt + 1)) {
      if (!line.text.trim().startsWith("|")) break;
      if (line.n === header.n + 1 && /^[\s|:-]*-[\s|:-]*$/.test(line.text)) continue;
      rows.push(line);
    }
  }
  for (const id of unitIds) {
    const named = new RegExp(`\\b${id}\\b`);
    if (!rows.some((row) => named.test(tableCells(row.text)[column.get("Scenario") ?? -1] ?? ""))) {
      problems.push(`${verifyPath}:${header?.n ?? 1} ${id} is missing a row`);
    }
  }
  for (const row of rows) {
    const cells = tableCells(row.text);
    const absent = verifyColumns.filter((name) => (column.get(name) ?? -1) >= cells.length);
    if (absent.length > 0) problems.push(`${verifyPath}:${row.n} row is missing the ${absent.join(", ")} cell${absent.length === 1 ? "" : "s"}`);
    for (const name of stampedColumns) {
      const cell = cells[column.get(name) ?? -1];
      if (cell === undefined) continue;
      if (/^n\/a\b/i.test(cell) && !/^n\/a:\s*\S/i.test(cell)) {
        problems.push(`${verifyPath}:${row.n} ${name} cell n/a is missing its reason`);
      } else if (final && cell === "") {
        problems.push(
          name === "Commit"
            ? `${verifyPath}:${row.n} Commit cell is missing a commit`
            : `${verifyPath}:${row.n} ${name} cell is missing evidence or n/a: <reason>`,
        );
      }
    }
  }
  return { problems, rows: rows.length, units: unitIds.length };
}

function fromRow(row: BundleRow): BundleRecord {
  return {
    id: row.id,
    state: row.state,
    pausedAt: row.paused_at ?? null,
    directory: row.directory,
    spec: row.spec,
    notes: row.notes,
    verify: row.verify,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function getBundle(context: PluginContext, id: string): BundleRecord | null {
  const row = context.store.database
    .query<BundleRow, [string]>("SELECT * FROM bundles WHERE id = ?")
    .get(id);
  return row ? fromRow(row) : null;
}

function requireBundle(context: PluginContext, id: string): BundleRecord {
  const bundle = getBundle(context, id);
  if (!bundle) {
    throw new CliError("NOT_FOUND", `bundle not found: ${id}; run: maestro bundle list`, {
      command: "maestro bundle list",
      id,
    });
  }
  return bundle;
}

function linkedWorkIds(context: PluginContext, id: string): string[] {
  return context.store.database
    .query<{ work_id: string }, [string]>(
      "SELECT work_id FROM bundle_work WHERE bundle_id = ? ORDER BY work_id",
    )
    .all(id)
    .map((row) => row.work_id);
}

async function readTrio(
  directory: string,
): Promise<{ spec: string | null; notes: string | null; verify: string | null }> {
  const [spec, notes, verify] = await Promise.all(
    trioFiles.map(async (name) => {
      const file = Bun.file(join(directory, name));
      return (await file.exists()) ? file.text() : null;
    }),
  );
  return { spec: spec ?? null, notes: notes ?? null, verify: verify ?? null };
}

function snapshotText(bundle: {
  id: string;
  spec: string | null;
  notes: string | null;
  verify: string | null;
}): string {
  return [bundle.id, bundle.spec, bundle.notes, bundle.verify]
    .filter((part): part is string => part !== null && part !== "")
    .join("\n");
}

function indexSnapshot(context: PluginContext, bundle: BundleRecord): void {
  if (!tableExists(context.store.database, "search_index")) return;
  context.store.database
    .query("DELETE FROM search_index WHERE surface = 'bundle' AND entity_id = ?")
    .run(bundle.id);
  context.store.database
    .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('bundle', ?, ?)")
    .run(bundle.id, snapshotText(bundle));
}

function decisionsForWork(
  context: PluginContext,
  workIds: string[],
): Array<{ id: string; state: string; text: string }> {
  if (workIds.length === 0) return [];
  const placeholders = workIds.map(() => "?").join(", ");
  return context.store.database
    .query<{ id: string; state: string; text: string }, string[]>(
      `SELECT id, state, text FROM decisions WHERE work_id IN (${placeholders})
        ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)`,
    )
    .all(...workIds);
}

// A design walk in the Hub room leaves its decisions in the Hub store while the
// build bundle opens where the code lives (d822); the SPEC lists them as
// hub:<id> and this read resolves them without a link table.
function hubDecisionsInSpec(
  context: PluginContext,
  spec: string | null,
): Array<{ id: string; state: string | null; text: string | null }> {
  const ids = [...new Set([...(spec ?? "").matchAll(/\bhub:(d\d+)\b/g)].map((match) => match[1] as string))];
  if (ids.length === 0) return [];
  const hub = resolveHubRoom();
  const lookup = (database: Database): Map<string, { state: string; text: string }> => {
    if (!tableExists(database, "decisions")) return new Map();
    const placeholders = ids.map(() => "?").join(", ");
    return new Map(
      database
        .query<{ id: string; state: string; text: string }, string[]>(
          `SELECT id, state, text FROM decisions WHERE id IN (${placeholders})`,
        )
        .all(...ids)
        .map((row) => [row.id, { state: row.state, text: row.text }]),
    );
  };
  let found = new Map<string, { state: string; text: string }>();
  if (samePath(context.store.path, hub.storePath)) {
    found = lookup(context.store.database);
  } else if (existsSync(hub.storePath)) {
    const database = new Database(hub.storePath, { readonly: true });
    try {
      found = lookup(database);
    } finally {
      database.close();
    }
  }
  return ids.map((id) => ({ id, state: found.get(id)?.state ?? null, text: found.get(id)?.text ?? null }));
}

function failedNotesForWork(
  context: PluginContext,
  workIds: string[],
): Array<{ text: string; workId: string }> {
  if (workIds.length === 0) return [];
  const placeholders = workIds.map(() => "?").join(", ");
  return context.store.database
    .query<{ text: string; work_id: string }, string[]>(
      `SELECT work_id, text FROM work_notes
       WHERE work_id IN (${placeholders}) AND SUBSTR(text, 1, 8) = 'failed: '
       ORDER BY id`,
    )
    .all(...workIds)
    .map((row) => ({ text: row.text, workId: row.work_id }));
}

function handbacksForWork(context: PluginContext, workIds: string[]): HandbackRecord[] {
  const dispatch = context.dispatch as DispatchService;
  const handback = context.handback as HandbackService;
  return workIds.flatMap((workId) =>
    dispatch.list(workId).flatMap((record) =>
      dispatch.council(workId, record.id).sealed ? [] : handback.list(record.id)
    )
  );
}

interface NotesSection {
  body: string;
  bodyEnd: number;
  bodyStart: number;
  name: string;
}

function notesSections(notes: string): NotesSection[] {
  const headings = [...notes.matchAll(/^## ([^\n]+)\n/gm)];
  return headings.map((heading, index) => {
    const bodyStart = (heading.index ?? 0) + heading[0].length;
    const bodyEnd = headings[index + 1]?.index ?? notes.length;
    return {
      body: notes.slice(bodyStart, bodyEnd),
      bodyEnd,
      bodyStart,
      name: heading[1] ?? "",
    };
  });
}

function handoffPlaceholders(notes: string | null): {
  any: boolean;
  sections: HandoffSectionName[];
} {
  if (!notes) return { any: false, sections: [] };
  const sections = notesSections(notes);
  return {
    any: notes.includes(handoffPlaceholder),
    sections: handoffSectionNames.filter((name) =>
      sections.some(
        (section) => section.name === name && section.body.includes(handoffPlaceholder),
      )
    ),
  };
}

function bundleOwnerSession(context: PluginContext, id: string): string | null {
  return context.log
    .list("bundle", id)
    .filter((event) => event.type === "bundle.open" || event.type === "bundle.resume")
    .at(-1)?.sessionId ?? null;
}

function requireCompleteHandoff(id: string, notes: string | null): void {
  const incomplete = handoffPlaceholders(notes).sections;
  if (incomplete.length === 0) return;
  const command = `maestro bundle open ${id}`;
  throw new CliError(
    "HANDOFF_INCOMPLETE",
    `${id} NOTES.md handoff sections are incomplete: ${incomplete.join(", ")}; replace every handoff placeholder, then run: ${command}`,
    { command, id, sections: incomplete },
  );
}

function trioSections(trio: {
  notes: string | null;
  spec: string | null;
  verify: string | null;
}): string[] {
  const parts = [trio.spec, trio.notes, trio.verify];
  return trioFiles.flatMap((name, index) => {
    const text = parts[index];
    return typeof text === "string" ? [`--- ${name}\n${text.trimEnd()}`] : [];
  });
}

function scaffoldBodies(id: string): Map<HandoffSectionName, string> {
  const sections = notesSections(notesTemplate(id, "Base:"));
  return new Map(
    sections
      .filter((section): section is NotesSection & { name: HandoffSectionName } =>
        handoffSectionNames.includes(section.name as HandoffSectionName)
      )
      .map((section) => [section.name, section.body]),
  );
}

function replaceUntouchedSections(
  id: string,
  notes: string,
  content: Map<HandoffSectionName, string>,
): { leftAlone: HandoffSectionName[]; notes: string; written: HandoffSectionName[] } {
  const scaffold = scaffoldBodies(id);
  const sections = notesSections(notes);
  const replacements = new Map<number, string>();
  const written: HandoffSectionName[] = [];
  const leftAlone: HandoffSectionName[] = [];
  for (const name of handoffSectionNames) {
    const section = sections.find((candidate) => candidate.name === name);
    if (!section || section.body !== scaffold.get(name)) {
      leftAlone.push(name);
      continue;
    }
    replacements.set(section.bodyStart, `\n${content.get(name) ?? handoffPlaceholder}\n\n`);
    written.push(name);
  }
  let output = "";
  let cursor = 0;
  for (const section of sections) {
    const replacement = replacements.get(section.bodyStart);
    if (replacement === undefined) continue;
    output += notes.slice(cursor, section.bodyStart) + replacement;
    cursor = section.bodyEnd;
  }
  return { leftAlone, notes: output + notes.slice(cursor), written };
}

// Hub finding F4 (w734): a bundle links work in either table, so work_id
// carries no foreign key to work(id); bundle_id still references bundles(id)
// and the row shape (bundle_id, work_id) and its primary key are unchanged.
const createBundleWorkTableSql = `CREATE TABLE IF NOT EXISTS bundle_work (
        bundle_id TEXT NOT NULL REFERENCES bundles(id),
        work_id TEXT NOT NULL,
        PRIMARY KEY(bundle_id, work_id)
      );`;

function bundleWorkReferencesWork(context: PluginContext): boolean {
  const schema = context.store.database
    .query<{ sql: string | null }, []>(
      "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'bundle_work'",
    )
    .get()?.sql;
  return schema?.includes("REFERENCES work(id)") ?? false;
}

// SQLite cannot drop a foreign key in place, so an existing table is rebuilt
// once, rows copied, the way the handbacks status vocabulary was widened.
function migrateBundleWorkWorkIdForeignKey(context: PluginContext): void {
  if (context.store.readOnly || !bundleWorkReferencesWork(context)) return;
  const migration = context.store.database.transaction(() => {
    if (!bundleWorkReferencesWork(context)) return;
    context.store.database.exec("ALTER TABLE bundle_work RENAME TO bundle_work_legacy_work_fk");
    context.store.database.exec(createBundleWorkTableSql);
    context.store.database.exec("INSERT INTO bundle_work SELECT * FROM bundle_work_legacy_work_fk");
    context.store.database.exec("DROP TABLE bundle_work_legacy_work_fk");
  });
  migration.immediate();
}

// Hub finding F4 (w734): an SLP generation writes its items to slp_work in the
// same project store, so a bundle may be linked to an id that the non-SLP work
// table does not hold. A store that has never run SLP has no slp_work table.
function knownWorkId(context: PluginContext, work: WorkService, workId: string): boolean {
  if (work.get(workId)) return true;
  if (!tableExists(context.store.database, "slp_work")) return false;
  return (
    context.store.database
      .query<{ present: number }, [string]>("SELECT 1 AS present FROM slp_work WHERE id = ?")
      .get(workId) !== null
  );
}

function replaceScaffoldBase(
  notes: string,
  base: string,
): { notes: string; written: boolean } {
  const match = notes.match(/^Base:$/m);
  if (!match) return { notes, written: false };
  return { notes: notes.replace(match[0], base), written: true };
}

function handoffContent(
  context: PluginContext,
  workIds: string[],
  handbacks: HandbackRecord[],
): Map<HandoffSectionName, string> {
  const work = context.work as WorkService;
  const byId = new Map(work.snapshot().map((record) => [record.id, record]));
  const workLines = workIds.flatMap((workId) => {
    const record = byId.get(workId);
    return record
      ? [
          `- ${record.id} [${record.state}] ${record.title}`,
          `  evidence: ${record.evidence || "none recorded"}`,
        ]
      : [];
  });
  const decisionLines = decisionsForWork(context, workIds).map(
    (decision) => `- ${decision.id} [${decision.state}] ${decision.text}`,
  );
  const handbackLines = handbacks.flatMap((handback) => [
    `- ${handback.id} [${handback.status}] dispatch ${handback.dispatchId}`,
    `  claim: ${handback.claim}`,
    `  proof: ${handback.proof}`,
    `  assumptions not verified: ${handback.assumptions}`,
    `  residual risks: ${handback.residualRisks}`,
    `  incidental findings: ${handback.incidentalFindings}`,
  ]);
  const currentState = [
    ...(workLines.length > 0 ? ["Work:", ...workLines] : []),
    ...(workLines.length > 0 && decisionLines.length > 0 ? [""] : []),
    ...(decisionLines.length > 0 ? ["Decisions:", ...decisionLines] : []),
    ...((workLines.length > 0 || decisionLines.length > 0) && handbackLines.length > 0 ? [""] : []),
    ...(handbackLines.length > 0 ? ["Handbacks:", ...handbackLines] : []),
  ];
  const failed = failedNotesForWork(context, workIds).map(
    (note) => `- ${note.workId}: ${note.text}`,
  );
  const checkpoints = latestCheckpoints(context.store.database, workIds);
  const checkpointLines = (field: "avoid" | "next" | "state") =>
    workIds.flatMap((workId) => {
      const value = checkpoints.get(workId)?.[field];
      return value ? [`- ${workId}: ${value}`] : [];
    });
  const stateLines = checkpointLines("state");
  if (stateLines.length > 0) {
    if (currentState.length > 0) currentState.push("");
    currentState.push("Checkpoint:", ...stateLines);
  }
  return new Map([
    ["Current State", currentState.join("\n") || handoffPlaceholder],
    ["Next Action", checkpointLines("next").join("\n") || handoffPlaceholder],
    ["Authority", handoffPlaceholder],
    ["Failed approaches", failed.join("\n") || handoffPlaceholder],
    ["Do not repeat", checkpointLines("avoid").join("\n") || handoffPlaceholder],
  ]);
}

function headline(bundle: BundleRecord): string {
  const state = bundle.state === "active" && bundle.pausedAt ? "paused" : bundle.state;
  return `${bundle.id} [${state}] ${bundle.directory}`;
}

export const bundlePlugin: BuiltInPlugin = {
  name: "bundle",
  inject: ["work", "decision", "dispatch", "handback"],
  requires:
    "bundle open/close/list/show/save: scaffold a SPEC/NOTES/VERIFY trio, snapshot it into the store on close, recall it via search and bundle show",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS bundles (
        id TEXT PRIMARY KEY,
        state TEXT NOT NULL CHECK(state IN ('active', 'archived')),
        directory TEXT NOT NULL,
        spec TEXT,
        notes TEXT,
        verify TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      ${createBundleWorkTableSql}
    `);
    migrateBundleWorkWorkIdForeignKey(context);
    // A paused bundle stays active in the state machine and only carries the
    // pause stamp (d778: waymark paused -> paused); the CHECK is left alone.
    context.store.ensureColumn("bundles", "paused_at", "ALTER TABLE bundles ADD COLUMN paused_at TEXT");

    context.effect(() =>
      registerSessionCommand(
        context,
        "bundle open",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          const existing = getBundle(context, id);
          const root = resolveStoreLocation(process.cwd()).root;
          const directory = existing?.directory ?? join(root, ".maestro", "bundle", id);
          const currentSession = context.sessions.current().id;
          const ownerSession = existing?.state === "active"
            ? bundleOwnerSession(context, id)
            : null;
          if (
            existing?.state === "active" && existsSync(directory) && ownerSession &&
            ownerSession !== currentSession
          ) {
            const trio = await readTrio(directory);
            requireCompleteHandoff(id, trio.notes);
            const workIds = linkedWorkIds(context, id);
            context.log.append({
              type: "bundle.resume",
              entityType: "bundle",
              entityId: id,
              sessionId: currentSession,
              payload: { directory },
            });
            return {
              data: { bundle: existing, workIds },
              text: [headline(existing), ...trioSections(trio)].join("\n"),
            };
          }
          if (existing) {
            throw new CliError("DUPLICATE", `bundle already exists: ${id}`, { id });
          }
          const workIds = stringOptions(invocation, "work");
          const work = context.work as WorkService;
          for (const workId of workIds) {
            if (!knownWorkId(context, work, workId)) {
              throw new CliError("NOT_FOUND", `work not found: ${workId}`);
            }
          }
          mkdirSync(directory, { recursive: true });
          const templates = {
            "SPEC.md": specTemplate(id),
            "NOTES.md": notesTemplate(id, await baseLine(root)),
            "VERIFY.md": verifyTemplate(id),
          };
          for (const name of trioFiles) {
            const path = join(directory, name);
            if (!existsSync(path)) await Bun.write(path, templates[name]);
          }
          const now = new Date().toISOString();
          const transaction = context.store.database.transaction(() => {
            context.store.database
              .query(
                `INSERT INTO bundles (id, state, directory, created_at, updated_at)
                 VALUES (?, 'active', ?, ?, ?)`,
              )
              .run(id, directory, now, now);
            for (const workId of workIds) {
              context.store.database
                .query("INSERT INTO bundle_work (bundle_id, work_id) VALUES (?, ?)")
                .run(id, workId);
            }
          });
          transaction();
          context.log.append({
            type: "bundle.open",
            entityType: "bundle",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { directory, workIds },
          });
          // w740 (room d153, d898): opening a Full-tier bundle on a work item
          // is the store's signal that a design phase has begun. The listener
          // is the SLP plugin, which owns every push line; this stays a plain
          // event so the bundle plugin keeps knowing nothing about panes. Only
          // this path fires it: a resume returned above, and a second open of
          // the same id threw DUPLICATE, so one design wakes a reviewer once.
          await context.events.emit<{ bundleId: string; workIds: string[] }>(
            "bundle.open",
            { bundleId: id, workIds },
          );
          const bundle = getBundle(context, id) as BundleRecord;
          return {
            data: { bundle, workIds },
            text: [
              headline(bundle),
              ...trioFiles.map((name) => `wrote: ${join(directory, name)}`),
              workIds.length > 0 ? `work: ${workIds.join(", ")}` : null,
            ]
              .filter((line): line is string => line !== null)
              .join("\n"),
          };
        },
        {
          description: "Scaffold a SPEC/NOTES/VERIFY bundle and record it as active.",
          flags: {
            "--work": {
              description: "Link a work item to this bundle.",
              value: true,
              multiple: true,
            },
          },
          positionals: [{ name: "id", required: true }],
          rootDescription: "Durable design bundles: scaffold, snapshot, recall.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "bundle close",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          const bundle = requireBundle(context, id);
          if (bundle.state !== "active") {
            throw new CliError("INVALID_STATE", `${id} is ${bundle.state}`);
          }
          const gate = await context.events.waterfall<
            { bundleId: string; sessionId: string },
            { blocked: boolean; origin?: string; reason?: string }
          >(
            "bundle.close",
            { bundleId: id, sessionId: context.sessions.current().id },
            async () => ({ blocked: false }),
          );
          if (gate.blocked) {
            throw new CliError("GATE_BLOCKED", gate.reason ?? "bundle.close gate blocked", {
              origin: gate.origin ?? "unknown",
            });
          }
          const trio = await readTrio(bundle.directory);
          if (handoffPlaceholders(trio.notes).any) {
            const command = `maestro bundle close ${id}`;
            throw new CliError(
              "HANDOFF_INCOMPLETE",
              `${id} NOTES.md still contains ${handoffPlaceholder}; replace every handoff placeholder, then run: ${command}`,
              { command, id },
            );
          }
          const now = new Date().toISOString();
          context.store.database
            .query(
              `UPDATE bundles SET state = 'archived', spec = ?, notes = ?, verify = ?,
                 updated_at = ? WHERE id = ?`,
            )
            .run(trio.spec, trio.notes, trio.verify, now, id);
          const archived = getBundle(context, id) as BundleRecord;
          indexSnapshot(context, archived);
          context.log.append({
            type: "bundle.close",
            entityType: "bundle",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { directory: bundle.directory },
          });
          const workIds = linkedWorkIds(context, id);
          const decisions = decisionsForWork(context, workIds);
          const hint = decisions.length === 0
            ? 'hint: no decisions link this bundle\'s work; record them via maestro decision draft "<text>" --rationale "<why>" --work <id>'
            : null;
          return {
            data: { bundle: archived },
            text: [headline(archived), `snapshot: ${trioFiles.join(", ")}`, hint]
              .filter((line): line is string => line !== null)
              .join("\n"),
          };
        },
        {
          description: "Snapshot the trio text into the store and archive the bundle.",
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "handoff",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          const work = context.work as WorkService;
          if (work.snapshot().some((record) => record.id === id)) {
            const command = "maestro bundle list";
            throw new CliError(
              "INVALID_TARGET",
              `handoff is bundle-scoped; ${id} is a work id; run: ${command}`,
              { command, id },
            );
          }
          const bundle = requireBundle(context, id);
          if (bundle.state !== "active") {
            throw new CliError(
              "INVALID_STATE",
              `${id} is ${bundle.state}; handoff requires an active bundle`,
            );
          }
          const notesPath = join(bundle.directory, "NOTES.md");
          const file = Bun.file(notesPath);
          if (!(await file.exists())) {
            throw new CliError("NOT_FOUND", `NOTES.md not found: ${notesPath}`, { path: notesPath });
          }
          const original = await file.text();
          const workIds = linkedWorkIds(context, id);
          const handbacks = handbacksForWork(context, workIds);
          const sectionResult = replaceUntouchedSections(
            id,
            original,
            handoffContent(context, workIds, handbacks),
          );
          const baseResult = replaceScaffoldBase(
            sectionResult.notes,
            await baseLine(resolveStoreLocation(process.cwd()).root),
          );
          const written = [
            ...(baseResult.written ? ["Base"] : []),
            ...sectionResult.written,
          ];
          const leftAlone = [
            ...(!baseResult.written ? ["Base"] : []),
            ...sectionResult.leftAlone,
          ];
          if (baseResult.notes !== original) await Bun.write(notesPath, baseResult.notes);
          context.log.append({
            type: "bundle.handoff",
            entityType: "bundle",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { leftAlone, written },
          });
          return {
            data: { bundle, leftAlone, notesPath, written, handbacks },
            text: [
              `${id} handoff: ${notesPath}`,
              `wrote: ${written.join(", ") || "none"}`,
              `left alone: ${leftAlone.join(", ") || "none"}`,
            ].join("\n"),
          };
        },
        {
          description: "Seed untouched NOTES.md sections from store and git evidence.",
          positionals: [{ name: "bundle-id", required: true }],
          rootDescription: "Prepare a factual bundle handoff without overwriting human notes.",
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "bundle save",
        async (invocation): Promise<CliResult> => {
          const directory = resolve(requiredPosition(invocation, 0, "bundle directory"));
          const id = basename(directory);
          if (getBundle(context, id)) {
            throw new CliError("DUPLICATE", `bundle already exists: ${id}`, { id });
          }
          const trio = await readTrio(directory);
          if (trio.spec === null && trio.notes === null && trio.verify === null) {
            throw new CliError(
              "NOT_FOUND",
              `no SPEC.md/NOTES.md/VERIFY.md found in: ${directory}`,
              { directory },
            );
          }
          const now = new Date().toISOString();
          context.store.database
            .query(
              `INSERT INTO bundles (id, state, directory, spec, notes, verify, created_at, updated_at)
               VALUES (?, 'archived', ?, ?, ?, ?, ?, ?)`,
            )
            .run(id, directory, trio.spec, trio.notes, trio.verify, now, now);
          const saved = getBundle(context, id) as BundleRecord;
          indexSnapshot(context, saved);
          context.log.append({
            type: "bundle.save",
            entityType: "bundle",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { directory },
          });
          return { data: { bundle: saved }, text: headline(saved) };
        },
        {
          description: "Ingest a foreign SPEC/NOTES/VERIFY dir straight to archived.",
          positionals: [{ name: "directory", required: true }],
        },
      ),
    );

    context.effect(() =>
      registerSessionCommand(
        context,
        "bundle import",
        (invocation): CliResult => {
          const source = resolve(requiredPosition(invocation, 0, "source directory"));
          if (!existsSync(source)) {
            throw new CliError("NOT_FOUND", `directory not found: ${source}`, { directory: source });
          }
          const dryRun = invocation.options["dry-run"] === true;
          const target = resolveStoreLocation(process.cwd()).root;
          const report = importWaymarkTree(context, source, target, dryRun);
          if (!dryRun) {
            context.log.append({
              type: "bundle.import",
              entityType: "bundle",
              entityId: basename(source),
              sessionId: context.sessions.current().id,
              payload: { counts: report.counts, source, target },
            });
          }
          return { data: report, text: formatImportReport(report) };
        },
        {
          description: "Import a .waymark/ tree: active and paused bundles copied, archives snapshotted, MEMORY.md and adr/ to decisions, CONTEXT.md to terms; every source item reported.",
          flags: { "--dry-run": { description: "Report the mapping without writing." } },
          positionals: [{ name: "directory", required: true }],
        },
      ),
    );

    for (const verb of ["pause", "resume"] as const) {
      context.effect(() =>
        registerSessionCommand(
          context,
          `bundle ${verb}`,
          (invocation): CliResult => {
            const id = requiredPosition(invocation, 0, "bundle id");
            const bundle = requireBundle(context, id);
            if (bundle.state !== "active") {
              throw new CliError("INVALID_STATE", `${id} is ${bundle.state}`, { id, state: bundle.state });
            }
            if (verb === "pause" && bundle.pausedAt) {
              throw new CliError("INVALID_STATE", `${id} is already paused`, { id });
            }
            if (verb === "resume" && !bundle.pausedAt) {
              throw new CliError("INVALID_STATE", `${id} is not paused`, { id });
            }
            const now = new Date().toISOString();
            context.store.database
              .query("UPDATE bundles SET paused_at = ?, updated_at = ? WHERE id = ?")
              .run(verb === "pause" ? now : null, now, id);
            const reason = invocation.options.reason;
            context.log.append({
              type: `bundle.${verb}`,
              entityType: "bundle",
              entityId: id,
              sessionId: context.sessions.current().id,
              payload: { reason: typeof reason === "string" ? reason : null },
            });
            const updated = getBundle(context, id) as BundleRecord;
            return { data: { bundle: updated }, text: headline(updated) };
          },
          {
            description: verb === "pause"
              ? "Pause an active bundle: it stays open but out of the way."
              : "Resume a paused bundle.",
            flags: verb === "pause" ? { "--reason": { description: "Why the bundle pauses.", value: true } } : {},
            positionals: [{ name: "id", required: true }],
          },
        ),
      );
    }

    context.effect(() =>
      context.cli.register(
        "bundle list",
        (): CliResult => {
          const bundles = context.store.database
            .query<BundleRow, []>("SELECT * FROM bundles ORDER BY created_at")
            .all()
            .map(fromRow);
          return {
            data: { bundles },
            text: bundles.length > 0
              ? bundles.map(headline).join("\n")
              : 'no bundles; run: maestro bundle open "<id>"',
          };
        },
        { description: "List bundles and their states.", mutates: false },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "bundle show",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          const bundle = requireBundle(context, id);
          const trio = bundle.state === "active"
            ? await readTrio(bundle.directory)
            : { spec: bundle.spec, notes: bundle.notes, verify: bundle.verify };
          const ownerSession = bundle.state === "active"
            ? bundleOwnerSession(context, id)
            : null;
          if (ownerSession && ownerSession !== context.sessions.current().id) {
            requireCompleteHandoff(id, trio.notes);
          }
          const workIds = linkedWorkIds(context, id);
          const work = context.work as WorkService;
          const workLines = workIds
            .map((workId) => work.get(workId))
            .filter((record): record is NonNullable<typeof record> => record !== null)
            .map((record) => `${record.id} [${record.state}] ${record.title}`);
          const decisions = decisionsForWork(context, workIds);
          const hubDecisions = hubDecisionsInSpec(context, trio.spec);
          const decisionLines = [
            ...decisions.map((decision) => `${decision.id} [${decision.state}] ${decision.text}`),
            ...hubDecisions.map((decision) =>
              decision.state === null
                ? `hub:${decision.id} not found in the Hub store`
                : `hub:${decision.id} [${decision.state}] ${decision.text}`
            ),
          ];
          const sections = trioSections(trio);
          return {
            data: { bundle, decisions, hubDecisions, workIds },
            text: [
              headline(bundle),
              ...sections,
              workLines.length > 0 ? `--- work\n${workLines.join("\n")}` : null,
              decisionLines.length > 0 ? `--- decisions\n${decisionLines.join("\n")}` : null,
            ]
              .filter((line): line is string => line !== null)
              .join("\n"),
          };
        },
        {
          description: "Compose one bundle: trio text, linked work, decisions.",
          mutates: false,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "bundle check",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          const bundle = requireBundle(context, id);
          const trio = bundle.state === "active"
            ? await readTrio(bundle.directory)
            : { spec: bundle.spec, notes: bundle.notes, verify: bundle.verify };
          const final = invocation.options.final === true;
          const specPath = join(bundle.directory, "SPEC.md");
          const result = checkPlan(
            trio.spec ?? "",
            trio.verify ?? "",
            specPath,
            join(bundle.directory, "VERIFY.md"),
            final,
          );
          if (!result) {
            return {
              data: { final, id, problems: [], rows: 0, units: 0, unitsSection: false },
              text: `bundle check ${id}: SPEC.md has no ## Units section; nothing to check`,
            };
          }
          const { problems, rows, units } = result;
          if (problems.length > 0) {
            throw new CliError("BUNDLE_CHECK_PROBLEMS", problems.join("\n"), { final, id, problems });
          }
          return {
            data: { final, id, problems, rows, units, unitsSection: true },
            text: `bundle check ${id}${final ? " --final" : ""}: OK (${units} unit${units === 1 ? "" : "s"}, ${rows} VERIFY row${rows === 1 ? "" : "s"})`,
          };
        },
        {
          description: "Lint SPEC units and VERIFY rows for missing cells; --final adds the verify-time checks.",
          flags: {
            "--final": {
              description: "Also require every unit, live and perf cell to hold evidence or an n/a with its reason, and a filled Commit cell.",
            },
          },
          mutates: false,
          positionals: [{ name: "id", required: true }],
        },
      ),
    );
  },
};
