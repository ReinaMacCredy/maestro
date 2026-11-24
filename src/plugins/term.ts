import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { tableExists } from "../kernel/store.ts";

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

function indexTerm(context: PluginContext, term: TermRecord): void {
  if (!tableExists(context.store.database, "search_index")) return;
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
    "term list/show: a glossary that lives in the store and answers maestro search next to work and decisions (d782)",
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

    context.effect(() =>
      context.cli.register(
        "term list",
        (): CliResult => {
          const terms = listTerms(context);
          return {
            data: { terms },
            text: terms.length > 0
              ? terms.map(formatTerm).join("\n")
              : "no terms",
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
          mutates: false,
          positionals: [{ name: "name", required: true }],
        },
      ),
    );
  },
};
