import type { Store } from "./store.ts";

export interface SessionRecord {
  id: string;
  pid: number;
  lastEvent: string;
  lastSeen: string;
  live: boolean;
}

interface SessionRow {
  id: string;
  pid: number;
  last_event: string;
  last_seen: string;
}

export class Sessions {
  constructor(private readonly store: Store) {
    store.migrate(`
      CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        pid INTEGER NOT NULL,
        last_event TEXT NOT NULL,
        last_seen TEXT NOT NULL
      );
    `);
  }

  current(): { id: string; pid: number } {
    const id = process.env.MAESTRO_SESSION_ID || `pid-${process.pid}`;
    const parsed = Number(process.env.MAESTRO_SESSION_PID ?? process.pid);
    return { id, pid: Number.isInteger(parsed) && parsed > 0 ? parsed : process.pid };
  }

  record(event: string): SessionRecord {
    const current = this.current();
    const lastSeen = new Date().toISOString();
    this.store.database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pid = excluded.pid,
           last_event = excluded.last_event,
           last_seen = excluded.last_seen`,
      )
      .run(current.id, current.pid, event, lastSeen);
    return { ...current, lastEvent: event, lastSeen, live: this.isPidAlive(current.pid) };
  }

  get(id: string): SessionRecord | null {
    const row = this.store.database
      .query<SessionRow, [string]>("SELECT * FROM sessions WHERE id = ?")
      .get(id);
    return row ? this.fromRow(row) : null;
  }

  isAlive(id: string): boolean {
    const session = this.get(id);
    return session ? this.isPidAlive(session.pid) : false;
  }

  list(): SessionRecord[] {
    return this.store.database
      .query<SessionRow, []>("SELECT * FROM sessions ORDER BY id")
      .all()
      .map((row) => this.fromRow(row));
  }

  private fromRow(row: SessionRow): SessionRecord {
    return {
      id: row.id,
      pid: row.pid,
      lastEvent: row.last_event,
      lastSeen: row.last_seen,
      live: this.isPidAlive(row.pid),
    };
  }

  private isPidAlive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  }
}
