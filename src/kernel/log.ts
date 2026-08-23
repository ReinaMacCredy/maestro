import type { Store } from "./store.ts";

export interface LogEvent<T = unknown> {
  id: number;
  type: string;
  entityType: string | null;
  entityId: string | null;
  sessionId: string | null;
  payload: T;
  createdAt: string;
}

interface LogRow {
  id: number;
  type: string;
  entity_type: string | null;
  entity_id: string | null;
  session_id: string | null;
  payload: string;
  created_at: string;
}

export class EventLog {
  constructor(private readonly store: Store) {
    store.migrate(`
      CREATE TABLE IF NOT EXISTS event_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        type TEXT NOT NULL,
        entity_type TEXT,
        entity_id TEXT,
        session_id TEXT,
        payload TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE TRIGGER IF NOT EXISTS event_log_no_update
      BEFORE UPDATE ON event_log
      BEGIN
        SELECT RAISE(ABORT, 'event log is append-only');
      END;
      CREATE TRIGGER IF NOT EXISTS event_log_no_delete
      BEFORE DELETE ON event_log
      BEGIN
        SELECT RAISE(ABORT, 'event log is append-only');
      END;
    `);
  }

  append(input: {
    type: string;
    entityType?: string;
    entityId?: string;
    sessionId?: string;
    payload?: unknown;
  }): number {
    const createdAt = new Date().toISOString();
    this.store.database
      .query(
        `INSERT INTO event_log
          (type, entity_type, entity_id, session_id, payload, created_at)
         VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(
        input.type,
        input.entityType ?? null,
        input.entityId ?? null,
        input.sessionId ?? null,
        JSON.stringify(input.payload ?? {}),
        createdAt,
      );
    return Number(this.store.database.query<{ id: number }, []>("SELECT last_insert_rowid() AS id").get()?.id);
  }

  list(entityType?: string, entityId?: string): LogEvent[] {
    let rows: LogRow[];
    if (entityType && entityId) {
      rows = this.store.database
        .query<LogRow, [string, string]>(
          "SELECT * FROM event_log WHERE entity_type = ? AND entity_id = ? ORDER BY id",
        )
        .all(entityType, entityId);
    } else {
      rows = this.store.database.query<LogRow, []>("SELECT * FROM event_log ORDER BY id").all();
    }
    return rows.map((row) => ({
      id: row.id,
      type: row.type,
      entityType: row.entity_type,
      entityId: row.entity_id,
      sessionId: row.session_id,
      payload: JSON.parse(row.payload),
      createdAt: row.created_at,
    }));
  }
}
