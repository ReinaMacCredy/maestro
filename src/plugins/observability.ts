import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

interface SearchRow {
  surface: string;
  entity_id: string;
  text: string;
}

interface DisplayedSearchRow extends SearchRow {
  cardType?: string;
  status?: string;
  title?: string;
}

interface LegacyCardSummary {
  card_type: string;
  status: string;
  title: string;
}

const legacySearchResultLimit = 20;

function oneLine(value: string, limit: number): string {
  const compact = value.replaceAll(/\s+/g, " ").trim();
  return compact.length <= limit ? compact : `${compact.slice(0, limit - 1).trimEnd()}…`;
}

function required(invocation: CliInvocation, index: number, label: string): string {
  const value = invocation.positionals[index];
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing ${label}`);
  return value;
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
          const displayed: DisplayedSearchRow[] = [];
          const lines: string[] = [];
          let legacyMatches = 0;
          for (const match of matches) {
            if (match.surface !== "[legacy]") {
              displayed.push(match);
              lines.push(`${match.surface} ${match.entity_id}: ${match.text}`);
              continue;
            }
            legacyMatches += 1;
            if (legacyMatches > legacySearchResultLimit) continue;
            const card = context.store.database
              .query<LegacyCardSummary, [string]>(
                "SELECT card_type, status, title FROM legacy_cards WHERE id = ?",
              )
              .get(match.entity_id);
            const id = oneLine(match.entity_id, 120);
            const kind = oneLine(card?.card_type ?? "unknown", 40);
            const status = oneLine(card?.status ?? "unknown", 40);
            const title = oneLine(card?.title ?? match.entity_id, 120);
            const snippet = oneLine(match.text, 200);
            displayed.push({
              ...match,
              entity_id: id,
              text: snippet,
              cardType: kind,
              status,
              title,
            });
            lines.push(`[legacy] ${id} (${kind}, ${status}): ${title} — ${snippet}`);
          }
          const more = legacyMatches - legacySearchResultLimit;
          if (more > 0) lines.push(`${more} more — refine query`);
          return {
            data: { matches: displayed },
            text: lines.join("\n"),
          };
        },
        {
          description: "Search work, decisions, notes, and event history.",
          maxPositionals: 1,
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
          maxPositionals: 1,
        },
      ),
    );
  },
};
