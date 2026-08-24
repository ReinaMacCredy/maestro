import type { Store } from "./store.ts";

export type Harness = "claude" | "codex";

export interface SessionRecord {
  harness: Harness | null;
  id: string;
  pid: number;
  lastEvent: string;
  lastSeen: string;
  live: boolean;
}

interface SessionRow {
  harness: Harness | null;
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
        last_seen TEXT NOT NULL,
        harness TEXT CHECK(harness IN ('claude', 'codex'))
      );
    `);
    const columns = store.database.query<{ name: string }, []>("PRAGMA table_info(sessions)").all();
    if (!columns.some((column) => column.name === "harness")) {
      try {
        store.migrate(
          "ALTER TABLE sessions ADD COLUMN harness TEXT CHECK(harness IN ('claude', 'codex'))",
        );
      } catch (error) {
        const migrated = store.database
          .query<{ name: string }, []>("PRAGMA table_info(sessions)")
          .all()
          .some((column) => column.name === "harness");
        if (!migrated) throw error;
      }
    }
  }

  current(): { id: string; pid: number } {
    const explicitPid = Number(process.env.MAESTRO_SESSION_PID);
    const pid = Number.isInteger(explicitPid) && explicitPid > 0 ? explicitPid : this.hostPid();
    const environmentId =
      process.env.MAESTRO_SESSION_ID ||
      process.env.CODEX_SESSION_ID ||
      process.env.CODEX_THREAD_ID ||
      process.env.CLAUDE_CODE_SESSION_ID ||
      process.env.CLAUDE_SESSION_ID ||
      process.env.CURSOR_SESSION_ID;
    if (environmentId) return { id: environmentId, pid };
    const recorded = this.store.database
      .query<{ id: string }, [number]>(
        "SELECT id FROM sessions WHERE pid = ? ORDER BY last_seen DESC LIMIT 1",
      )
      .get(pid);
    return { id: recorded?.id ?? `pid-${pid}`, pid };
  }

  record(event: string, harness?: Harness | null): SessionRecord {
    const current = this.current();
    const lastSeen = new Date().toISOString();
    const recordedHarness = harness === undefined ? (this.get(current.id)?.harness ?? null) : harness;
    this.store.database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pid = excluded.pid,
           last_event = excluded.last_event,
           last_seen = excluded.last_seen,
           harness = excluded.harness`,
      )
      .run(current.id, current.pid, event, lastSeen, recordedHarness);
    return {
      ...current,
      harness: recordedHarness,
      lastEvent: event,
      lastSeen,
      live: this.isPidAlive(current.pid),
    };
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
      harness: row.harness,
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

  private hostPid(): number {
    const directParent = process.ppid;
    let candidate = directParent;
    for (let depth = 0; depth < 6 && candidate > 1; depth += 1) {
      let result: ReturnType<typeof Bun.spawnSync>;
      try {
        result = Bun.spawnSync(["ps", "-o", "ppid=,comm=", "-p", String(candidate)]);
      } catch {
        break;
      }
      if (result.exitCode !== 0) break;
      if (!result.stdout) break;
      const output = result.stdout.toString().trim();
      const match = output.match(/^(\d+)\s+(.+)$/);
      if (!match) break;
      const command = match[2] as string;
      if (/(^|\/)(codex|claude|cursor)(\s|$)/i.test(command)) return candidate;
      candidate = Number(match[1]);
    }
    return directParent;
  }
}
