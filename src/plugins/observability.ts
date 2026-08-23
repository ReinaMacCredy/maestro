import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

interface SearchRow {
  surface: string;
  entity_id: string;
  text: string;
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
      context.cli.register("search", (invocation): CliResult => {
        const term = required(invocation, 0, "search term");
        const query = `"${term.replaceAll('"', '""')}"`;
        const matches = context.store.database
          .query<SearchRow, [string]>(
            "SELECT surface, entity_id, text FROM search_index WHERE search_index MATCH ? ORDER BY rowid",
          )
          .all(query);
        return {
          data: { matches },
          text: matches
            .map((match) => `${match.surface} ${match.entity_id}: ${match.text}`)
            .join("\n"),
        };
      }, {}, 1),
    );

    context.effect(() =>
      context.cli.register("trace", (invocation): CliResult => {
        const id = required(invocation, 0, "work id");
        const events = context.log.list("work", id);
        return {
          data: { events },
          text: events
            .map((event) => `${event.id} ${event.type} ${JSON.stringify(event.payload)}`)
            .join("\n"),
        };
      }, {}, 1),
    );
  },
};
