import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";

interface SearchRow {
  surface: string;
  entity_id: string;
  text: string;
}

interface SearchSummary {
  id: string;
  kind: string;
  source: "legacy" | "native";
  state: string;
  title: string;
}

interface SearchHit {
  key: string;
  snippets: string[];
  summary: SearchSummary;
}

interface LegacyCardSummary {
  card_type: string;
  status: string;
  title: string;
}

interface WorkSummary {
  id: string;
  kind: string;
  state: string;
  title: string;
}

interface DecisionSummary {
  id: string;
  state: string;
  text: string;
}

interface EventOwner {
  entity_id: string | null;
  entity_type: string | null;
  type: string;
}

const searchResultLimit = 20;

function oneLine(value: string, limit: number): string {
  const compact = value.replaceAll(/\s+/g, " ").trim();
  return compact.length <= limit ? compact : `${compact.slice(0, limit - 1).trimEnd()}…`;
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
}

function workHit(context: PluginContext, id: string, snippet: string): SearchHit | null {
  const work = context.store.database
    .query<WorkSummary, [string]>("SELECT id, kind, state, title FROM work WHERE id = ?")
    .get(id);
  return work
    ? {
        key: `native:work:${work.id}`,
        snippets: snippet ? [snippet] : [],
        summary: {
          id: work.id,
          kind: work.kind,
          source: "native",
          state: work.state,
          title: work.title,
        },
      }
    : null;
}

function decisionHit(context: PluginContext, id: string, snippet: string): SearchHit | null {
  const decision = context.store.database
    .query<DecisionSummary, [string]>("SELECT id, state, text FROM decisions WHERE id = ?")
    .get(id);
  return decision
    ? {
        key: `native:decision:${decision.id}`,
        snippets: snippet ? [snippet] : [],
        summary: {
          id: decision.id,
          kind: "decision",
          source: "native",
          state: decision.state,
          title: decision.text,
        },
      }
    : null;
}

function nativeHit(context: PluginContext, match: SearchRow): SearchHit | null {
  if (match.surface === "work") return workHit(context, match.entity_id, match.text);
  if (match.surface === "decision") return decisionHit(context, match.entity_id, match.text);
  if (match.surface === "note") {
    const owner = context.store.database
      .query<{ work_id: string }, [number]>("SELECT work_id FROM work_notes WHERE id = ?")
      .get(Number(match.entity_id));
    return owner ? workHit(context, owner.work_id, match.text) : null;
  }
  if (match.surface !== "log") return null;
  const event = context.store.database
    .query<EventOwner, [number]>(
      "SELECT type, entity_type, entity_id FROM event_log WHERE id = ?",
    )
    .get(Number(match.entity_id));
  if (!event) return null;
  if (event.entity_type === "work" && event.entity_id) {
    return workHit(context, event.entity_id, "");
  }
  if (event.entity_type === "decision" && event.entity_id) {
    return decisionHit(context, event.entity_id, "");
  }
  const id = event.entity_id ?? `event-${match.entity_id}`;
  const kind = event.entity_type ?? "event";
  return {
    key: `native:${kind}:${id}`,
    snippets: [event.type],
    summary: { id, kind, source: "native", state: "event", title: event.type },
  };
}

function legacyHit(context: PluginContext, match: SearchRow): SearchHit {
  const card = context.store.database
    .query<LegacyCardSummary, [string]>(
      "SELECT card_type, status, title FROM legacy_cards WHERE id = ?",
    )
    .get(match.entity_id);
  return {
    key: `legacy:${match.entity_id}`,
    snippets: [match.text],
    summary: {
      id: match.entity_id,
      kind: card?.card_type ?? "unknown",
      source: "legacy",
      state: card?.status ?? "unknown",
      title: card?.title ?? match.entity_id,
    },
  };
}

function mergeHit(hits: Map<string, SearchHit>, incoming: SearchHit): void {
  const existing = hits.get(incoming.key);
  if (!existing) {
    hits.set(incoming.key, incoming);
    return;
  }
  for (const snippet of incoming.snippets) {
    if (!existing.snippets.includes(snippet)) existing.snippets.push(snippet);
  }
}

function formatHit(hit: SearchHit): string {
  const prefix = hit.summary.source === "legacy" ? "[legacy] " : "";
  const id = oneLine(hit.summary.id, 120);
  const kind = oneLine(hit.summary.kind, 40);
  const state = oneLine(hit.summary.state, 40);
  const title = oneLine(hit.summary.title, 120);
  const snippet = oneLine(hit.snippets.join(" | ") || hit.summary.title, 200);
  return `${prefix}${id} (${kind}, ${state}): ${title} — ${snippet}`;
}

export const observabilityPlugin: BuiltInPlugin = {
  name: "observability",
  inject: ["work", "decision"],
  apply(context) {
    context.store.migrate(`
      CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
        surface UNINDEXED,
        entity_id UNINDEXED,
        text
      );
      CREATE TRIGGER IF NOT EXISTS search_work_insert
      AFTER INSERT ON work BEGIN
        INSERT INTO search_index(surface, entity_id, text)
        VALUES ('work', new.id, new.title || ' ' || COALESCE(new.acceptance, '') || ' ' || COALESCE(new.evidence, ''));
      END;
      CREATE TRIGGER IF NOT EXISTS search_work_update
      AFTER UPDATE OF title, acceptance, evidence ON work BEGIN
        DELETE FROM search_index WHERE surface = 'work' AND entity_id = old.id;
        INSERT INTO search_index(surface, entity_id, text)
        VALUES ('work', new.id, new.title || ' ' || COALESCE(new.acceptance, '') || ' ' || COALESCE(new.evidence, ''));
      END;
      CREATE TRIGGER IF NOT EXISTS search_note_insert
      AFTER INSERT ON work_notes BEGIN
        INSERT INTO search_index(surface, entity_id, text) VALUES ('note', CAST(new.id AS TEXT), new.text);
      END;
      CREATE TRIGGER IF NOT EXISTS search_decision_insert
      AFTER INSERT ON decisions BEGIN
        INSERT INTO search_index(surface, entity_id, text) VALUES ('decision', new.id, new.text);
      END;
      CREATE TRIGGER IF NOT EXISTS search_decision_update
      AFTER UPDATE OF text ON decisions BEGIN
        DELETE FROM search_index WHERE surface = 'decision' AND entity_id = old.id;
        INSERT INTO search_index(surface, entity_id, text) VALUES ('decision', new.id, new.text);
      END;
      CREATE TRIGGER IF NOT EXISTS search_log_insert
      AFTER INSERT ON event_log BEGIN
        INSERT INTO search_index(surface, entity_id, text)
        VALUES ('log', CAST(new.id AS TEXT), new.type || ' ' || new.payload);
      END;
    `);
    context.store.database.run("DELETE FROM search_index");
    context.store.database.run(`
      INSERT INTO search_index(surface, entity_id, text)
      SELECT 'work', id, title || ' ' || COALESCE(acceptance, '') || ' ' || COALESCE(evidence, '') FROM work
    `);
    context.store.database.run(
      "INSERT INTO search_index(surface, entity_id, text) SELECT 'note', CAST(id AS TEXT), text FROM work_notes",
    );
    context.store.database.run(
      "INSERT INTO search_index(surface, entity_id, text) SELECT 'decision', id, text FROM decisions",
    );
    context.store.database.run(
      "INSERT INTO search_index(surface, entity_id, text) SELECT 'log', CAST(id AS TEXT), type || ' ' || payload FROM event_log",
    );

    context.effect(() =>
      context.cli.register(
        "search",
        (invocation): CliResult => {
          const term = required(invocation, 0, "search term");
          const query = `"${term.replaceAll('"', '""')}"`;
          const matches = context.store.database
            .query<SearchRow, [string]>(
              `SELECT surface, entity_id,
                      CASE WHEN surface = '[legacy]'
                        THEN snippet(search_index, 2, '', '', ' … ', 32)
                        ELSE text
                      END AS text
                 FROM search_index
                WHERE search_index MATCH ?
                ORDER BY rowid`,
            )
            .all(query);
          const hits = new Map<string, SearchHit>();
          for (const match of matches) {
            const hit = match.surface === "[legacy]"
              ? legacyHit(context, match)
              : nativeHit(context, match);
            if (hit) mergeHit(hits, hit);
          }
          const bounded = [...hits.values()].slice(0, searchResultLimit);
          const lines = bounded.map(formatHit);
          const more = hits.size - searchResultLimit;
          if (more > 0) lines.push(`${more} more — refine query`);
          return {
            data: { matches: bounded.map((hit) => hit.summary) },
            text: lines.join("\n"),
          };
        },
        {
          description: "Search work, decisions, notes, and event history.",
          positionals: [{ name: "query", required: true }],
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "trace",
        (invocation): CliResult => {
          const id = required(invocation, 0, "work id");
          const events = context.log.list("work", id);
          return {
            data: { events },
            text: events
              .map((event) => `${event.id} ${event.type} ${JSON.stringify(event.payload)}`)
              .join("\n"),
          };
        },
        {
          description: "Reconstruct one work item's event history.",
          positionals: [{ name: "id", required: true }],
        },
      ),
    );
  },
};
