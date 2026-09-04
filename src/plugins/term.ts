import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";

export interface TermRecord {
  id: string;
  name: string;
  definition: string;
  workId: string | null;
  createdAt: string;
  updatedAt: string;
}

interface TermRow {
  id: string;
  name: string;
  definition: string;
  work_id: string | null;
  created_at: string;
  updated_at: string;
}

function fromRow(row: TermRow): TermRecord {
  return {
    id: row.id,
    name: row.name,
    definition: row.definition,
    workId: row.work_id,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index]?.trim();
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function getTerm(context: PluginContext, key: string): TermRecord | null {
  const row = context.store.database
    .query<TermRow, [string, string]>("SELECT * FROM terms WHERE id = ? OR name = ?")
    .get(key, key);
  return row ? fromRow(row) : null;
}

function listTerms(context: PluginContext): TermRecord[] {
  return context.store.database
    .query<TermRow, []>("SELECT * FROM terms ORDER BY name")
    .all()
    .map(fromRow);
}

function hasSearchIndex(context: PluginContext): boolean {
  return context.store.database
    .query<{ present: number }, [string]>(
      "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get("search_index") !== null;
}

// Observability rebuilds search_index from its own tables only, so terms
// re-index themselves on apply and on every write (bundle precedent).
function indexTerm(context: PluginContext, term: TermRecord): void {
  if (!hasSearchIndex(context)) return;
  context.store.database
    .query("DELETE FROM search_index WHERE surface = 'term' AND entity_id = ?")
    .run(term.id);
  context.store.database
    .query("INSERT INTO search_index(surface, entity_id, text) VALUES ('term', ?, ?)")
    .run(term.id, `${term.name} ${term.definition}`);
}

export function formatTerm(term: TermRecord): string {
  return `${term.id} ${term.name}: ${term.definition}` + (term.workId ? ` (work ${term.workId})` : "");
}

export const termPlugin: BuiltInPlugin = {
  name: "term",
  inject: ["work"],
  requires:
    "term add/list/show: a glossary that lives in the store and answers maestro search next to work and decisions (d782)",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS terms (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        definition TEXT NOT NULL,
        work_id TEXT REFERENCES work(id),
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
    `);
    if (!context.store.readOnly) {
      for (const term of listTerms(context)) indexTerm(context, term);
    }

    context.effect(() =>
      registerSessionCommand(
        context,
        "term add",
        (invocation): CliResult => {
          const name = required(invocation, 0, "term name");
          const definition = required(invocation, 1, "term definition");
          if (/\s/.test(name)) {
            throw new CliError("INVALID_ARGUMENT", `term names carry no whitespace: ${name}`, {
              name,
            });
          }
          const workOption = invocation.options.work;
          const workId = typeof workOption === "string" ? workOption : null;
          if (workId && !context.store.database
            .query<{ id: string }, [string]>("SELECT id FROM work WHERE id = ?")
            .get(workId)) {
            throw new CliError("NOT_FOUND", `work not found: ${workId}`, { id: workId });
          }
          const now = new Date().toISOString();
          const existing = getTerm(context, name);
          const saved = context.store.database.transaction(() => {
            if (existing) {
              context.store.database
                .query("UPDATE terms SET definition = ?, work_id = COALESCE(?, work_id), updated_at = ? WHERE id = ?")
                .run(definition, workId, now, existing.id);
            } else {
              context.store.database
                .query(
                  "INSERT INTO terms(id, name, definition, work_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .run(context.store.nextPrefixedId("terms", "t"), name, definition, workId, now, now);
            }
            const term = getTerm(context, name) as TermRecord;
            indexTerm(context, term);
            context.log.append({
              type: existing ? "term.update" : "term.add",
              entityType: "term",
              entityId: term.id,
              sessionId: context.sessions.current().id,
              payload: { name, workId },
            });
            return term;
          })();
          return {
            data: { term: saved, updated: existing !== null },
            text: `${formatTerm(saved)}${existing ? " (definition replaced)" : ""}`,
          };
        },
        {
          description: "Record or redefine one glossary term.",
          flags: { "--work": { description: "Link the term to a work item.", value: true } },
          json: true,
          positionals: [
            { name: "name", required: true },
            { name: "definition", required: true },
          ],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "term list",
        (): CliResult => {
          const terms = listTerms(context);
          return {
            data: { terms },
            text: terms.length > 0
              ? terms.map(formatTerm).join("\n")
              : 'no terms; run: maestro term add "<name>" "<definition>"',
          };
        },
        { description: "List glossary terms.", mutates: false },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "term show",
        (invocation): CliResult => {
          const key = required(invocation, 0, "term name or id");
          const term = getTerm(context, key);
          if (!term) {
            throw new CliError("NOT_FOUND", `term not found: ${key}; run: maestro term list`, {
              command: "maestro term list",
              key,
            });
          }
          return { data: { term }, text: formatTerm(term) };
        },
        {
          description: "Show one glossary term.",
          json: true,
          mutates: false,
          positionals: [{ name: "name", required: true }],
        },
      ),
    );
  },
};
