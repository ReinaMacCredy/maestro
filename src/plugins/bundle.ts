import { existsSync, mkdirSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOptions,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";
import type {
  DispatchService,
  HandbackRecord,
  HandbackService,
} from "./dispatch.ts";
import { registerSessionCommand } from "./session-required.ts";
import type { WorkService } from "./work.ts";

export interface BundleRecord {
  id: string;
  state: "active" | "archived";
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

Mid-flight decisions: record via \`maestro decision draft "<text>" --rationale "<why>"\` and link the ids here instead of restating them.
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

Scenarios point at work acceptance/claims; do not restate them.

| # | Scenario | Work / acceptance | Result |
|---|----------|-------------------|--------|
`;
}

function fromRow(row: BundleRow): BundleRecord {
  return {
    id: row.id,
    state: row.state,
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

function hasSearchIndex(context: PluginContext): boolean {
  return context.store.database
    .query<{ present: number }, [string]>(
      "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get("search_index") !== null;
}

function indexSnapshot(context: PluginContext, bundle: BundleRecord): void {
  if (!hasSearchIndex(context)) return;
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
  return new Map([
    ["Current State", currentState.join("\n") || handoffPlaceholder],
    ["Next Action", handoffPlaceholder],
    ["Authority", handoffPlaceholder],
    ["Failed approaches", failed.join("\n") || handoffPlaceholder],
    ["Do not repeat", handoffPlaceholder],
  ]);
}

function headline(bundle: BundleRecord): string {
  return `${bundle.id} [${bundle.state}] ${bundle.directory}`;
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
      CREATE TABLE IF NOT EXISTS bundle_work (
        bundle_id TEXT NOT NULL REFERENCES bundles(id),
        work_id TEXT NOT NULL REFERENCES work(id),
        PRIMARY KEY(bundle_id, work_id)
      );
    `);
    // Observability rebuilds search_index from scratch each startup and only
    // knows its own tables; re-insert archived snapshots (import-rust precedent).
    if (!context.store.readOnly) {
      for (const row of context.store.database
        .query<BundleRow, []>("SELECT * FROM bundles WHERE state = 'archived'")
        .all()) {
        indexSnapshot(context, fromRow(row));
      }
    }

    context.effect(() =>
      registerSessionCommand(
        context,
        "bundle open",
        async (invocation): Promise<CliResult> => {
          const id = requiredPosition(invocation, 0, "bundle id");
          if (getBundle(context, id)) {
            throw new CliError("DUPLICATE", `bundle already exists: ${id}`, { id });
          }
          const workIds = stringOptions(invocation, "work");
          const work = context.work as WorkService;
          for (const workId of workIds) {
            if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          }
          const root = resolveStoreLocation(process.cwd()).root;
          const directory = join(root, ".maestro", "bundle", id);
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
          const trio = await readTrio(bundle.directory);
          if (trio.notes?.includes(handoffPlaceholder)) {
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
          const workIds = linkedWorkIds(context, id);
          const work = context.work as WorkService;
          const workLines = workIds
            .map((workId) => work.get(workId))
            .filter((record): record is NonNullable<typeof record> => record !== null)
            .map((record) => `${record.id} [${record.state}] ${record.title}`);
          const decisionLines = decisionsForWork(context, workIds).map(
            (decision) => `${decision.id} [${decision.state}] ${decision.text}`,
          );
          const parts = [trio.spec, trio.notes, trio.verify];
          const sections = trioFiles.flatMap((name, index) => {
            const text = parts[index];
            return typeof text === "string" ? [`--- ${name}\n${text.trimEnd()}`] : [];
          });
          return {
            data: { bundle, decisions: decisionsForWork(context, workIds), workIds },
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
  },
};
