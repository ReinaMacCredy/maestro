import { existsSync, mkdirSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";
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

function specTemplate(id: string): string {
  return `# SPEC — ${id}

## Problem

## Solution

## Scope

## Anti-goals

Mid-flight decisions: record via \`maestro decision draft "<text>" --rationale "<why>"\` and link the ids here instead of restating them.
`;
}

function notesTemplate(id: string): string {
  return `# NOTES — ${id}

Overwritten handoff, never appended; history lives in trace and decisions.

Base:

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

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function listOption(invocation: CliInvocation, name: string): string[] {
  const value = invocation.options[name];
  if (Array.isArray(value)) return value;
  return typeof value === "string" ? [value] : [];
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

function headline(bundle: BundleRecord): string {
  return `${bundle.id} [${bundle.state}] ${bundle.directory}`;
}

export const bundlePlugin: BuiltInPlugin = {
  name: "bundle",
  inject: ["work", "decision"],
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
    for (const row of context.store.database
      .query<BundleRow, []>("SELECT * FROM bundles WHERE state = 'archived'")
      .all()) {
      indexSnapshot(context, fromRow(row));
    }

    context.effect(() =>
      context.cli.register(
        "bundle open",
        async (invocation): Promise<CliResult> => {
          const id = required(invocation, 0, "bundle id");
          if (getBundle(context, id)) {
            throw new CliError("DUPLICATE", `bundle already exists: ${id}`, { id });
          }
          const workIds = listOption(invocation, "work");
          const work = context.work as WorkService;
          for (const workId of workIds) {
            if (!work.get(workId)) throw new CliError("NOT_FOUND", `work not found: ${workId}`);
          }
          const root = resolveStoreLocation(process.cwd()).root;
          const directory = join(root, ".maestro", "bundle", id);
          mkdirSync(directory, { recursive: true });
          const templates = {
            "SPEC.md": specTemplate(id),
            "NOTES.md": notesTemplate(id),
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
      context.cli.register(
        "bundle close",
        async (invocation): Promise<CliResult> => {
          const id = required(invocation, 0, "bundle id");
          const bundle = requireBundle(context, id);
          if (bundle.state !== "active") {
            throw new CliError("INVALID_STATE", `${id} is ${bundle.state}`);
          }
          const trio = await readTrio(bundle.directory);
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
      context.cli.register(
        "bundle save",
        async (invocation): Promise<CliResult> => {
          const directory = resolve(required(invocation, 0, "bundle directory"));
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
        { description: "List bundles and their states." },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "bundle show",
        async (invocation): Promise<CliResult> => {
          const id = required(invocation, 0, "bundle id");
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
          positionals: [{ name: "id", required: true }],
        },
      ),
    );
  },
};
