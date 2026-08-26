import type { Store } from "./store.ts";

export type Harness = "claude" | "codex";
export type SessionAnchor = "pid" | "ttl";

export const sessionTtlMs = 60 * 60 * 1000;

export interface SessionRecord {
  anchor: SessionAnchor;
  harness: Harness | null;
  id: string;
  pid: number;
  lastEvent: string;
  lastSeen: string;
  live: boolean;
}

interface SessionRow {
  anchor: SessionAnchor;
  harness: Harness | null;
  id: string;
  pid: number;
  last_event: string;
  last_seen: string;
  scope: string;
}

interface CurrentSession {
  anchor: SessionAnchor;
  harness: Harness | null;
  id: string;
  pid: number;
  scope: string;
}

export interface SessionLiveness {
  live: boolean;
  reason: string;
}

export class Sessions {
  private resolved: CurrentSession | null = null;
  private sharedPidCache: Set<number> | null = null;

  constructor(
    private readonly store: Store,
    private readonly scope = process.cwd(),
  ) {
    if (store.readOnly && !store.ephemeral) return;
    store.migrate(`
      CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        pid INTEGER NOT NULL,
        last_event TEXT NOT NULL,
        last_seen TEXT NOT NULL,
        harness TEXT CHECK(harness IN ('claude', 'codex')),
        anchor TEXT NOT NULL DEFAULT 'pid' CHECK(anchor IN ('pid', 'ttl')),
        scope TEXT NOT NULL DEFAULT ''
      );
    `);
    this.ensureColumn(
      "harness",
      "ALTER TABLE sessions ADD COLUMN harness TEXT CHECK(harness IN ('claude', 'codex'))",
    );
    this.ensureColumn(
      "anchor",
      "ALTER TABLE sessions ADD COLUMN anchor TEXT NOT NULL DEFAULT 'pid' CHECK(anchor IN ('pid', 'ttl'))",
    );
    this.ensureColumn(
      "scope",
      "ALTER TABLE sessions ADD COLUMN scope TEXT NOT NULL DEFAULT ''",
    );
  }

  current(): { id: string; pid: number } {
    const current = this.resolveCurrent();
    return { id: current.id, pid: current.pid };
  }

  refresh(): void {
    if (this.disabled()) return;
    const current = this.resolveCurrent();
    const recorded = this.row(current.id);
    if ((recorded?.anchor ?? current.anchor) === "pid") return;
    const lastSeen = new Date().toISOString();
    const harness = recorded?.harness ?? current.harness;
    this.store.database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, ?, ?, ?, 'ttl', ?)
         ON CONFLICT(id) DO UPDATE SET
           pid = excluded.pid,
           last_seen = excluded.last_seen,
           harness = COALESCE(sessions.harness, excluded.harness),
           anchor = excluded.anchor,
           scope = excluded.scope`,
      )
      .run(current.id, current.pid, recorded?.last_event ?? "Command", lastSeen, harness, current.scope);
  }

  record(event: string, harness?: Harness | null): SessionRecord {
    const current = this.resolveCurrent(harness);
    if (this.disabled()) {
      return {
        anchor: current.anchor,
        harness: current.harness,
        id: current.id,
        pid: current.pid,
        lastEvent: event,
        lastSeen: new Date().toISOString(),
        live: true,
      };
    }
    const recorded = this.row(current.id);
    const anchor = recorded?.anchor === "ttl" ? "ttl" : current.anchor;
    const lastSeen = new Date().toISOString();
    const recordedHarness = harness !== undefined
      ? harness === null && anchor === "ttl"
        ? (recorded?.harness ?? current.harness)
        : harness
      : (recorded?.harness ?? current.harness);
    this.store.database
      .query(
        `INSERT INTO sessions (id, pid, last_event, last_seen, harness, anchor, scope)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pid = excluded.pid,
           last_event = excluded.last_event,
           last_seen = excluded.last_seen,
           harness = excluded.harness,
           anchor = excluded.anchor,
           scope = excluded.scope`,
      )
      .run(
        current.id,
        current.pid,
        event,
        lastSeen,
        recordedHarness,
        anchor,
        current.scope,
      );
    const row: SessionRow = {
      anchor,
      harness: recordedHarness,
      id: current.id,
      pid: current.pid,
      last_event: event,
      last_seen: lastSeen,
      scope: current.scope,
    };
    return this.fromRow(row);
  }

  get(id: string): SessionRecord | null {
    const row = this.row(id);
    return row ? this.fromRow(row) : null;
  }

  isAlive(id: string): boolean {
    return this.liveness(id).live;
  }

  liveness(id: string): SessionLiveness {
    const row = this.row(id);
    return row
      ? this.rowLiveness(this.downgradeSharedPid(row))
      : { live: false, reason: `session ${id} is not recorded` };
  }

  list(): SessionRecord[] {
    return this.store.database
      .query<SessionRow, []>("SELECT * FROM sessions ORDER BY id")
      .all()
      .map((row) => this.fromRow(this.normalizeRow(row)));
  }

  private resolveCurrent(harness?: Harness | null): CurrentSession {
    if (this.resolved) {
      if (harness) this.resolved.harness = harness;
      return this.resolved;
    }
    if (this.disabled()) {
      this.resolved = {
        anchor: "pid",
        harness: null,
        id: "session-none",
        pid: process.pid,
        scope: this.scope,
      };
      return this.resolved;
    }
    const explicitPid = Number(process.env.MAESTRO_SESSION_PID);
    const hasExplicitPid = Number.isInteger(explicitPid) && explicitPid > 0;
    const host = hasExplicitPid
      ? { pid: explicitPid, verified: true }
      : this.hostPid();
    const anchor: SessionAnchor = host.verified ? "pid" : "ttl";
    const pid = host.pid;
    const environmentId =
      process.env.MAESTRO_SESSION_ID ||
      process.env.CODEX_SESSION_ID ||
      process.env.CODEX_THREAD_ID ||
      process.env.CLAUDE_CODE_SESSION_ID ||
      process.env.CLAUDE_SESSION_ID ||
      process.env.CURSOR_SESSION_ID;
    const guessedHarness = harness ?? this.guessHarness();
    if (environmentId) {
      this.resolved = { anchor, harness: guessedHarness, id: environmentId, pid, scope: this.scope };
      return this.resolved;
    }
    if (anchor === "pid") {
      const recorded = this.store.database
        .query<{ id: string }, [number]>(
          "SELECT id FROM sessions WHERE pid = ? ORDER BY last_seen DESC LIMIT 1",
        )
        .get(pid);
      this.resolved = {
        anchor,
        harness: guessedHarness,
        id: recorded?.id ?? `pid-${pid}`,
        pid,
        scope: this.scope,
      };
      return this.resolved;
    }
    const recorded = this.recentTtlSession(guessedHarness);
    this.resolved = {
      anchor,
      harness: recorded?.harness ?? guessedHarness,
      id: recorded?.id ?? `ttl-${crypto.randomUUID()}`,
      pid,
      scope: this.scope,
    };
    return this.resolved;
  }

  private fromRow(row: SessionRow): SessionRecord {
    const current = this.downgradeSharedPid(row);
    return {
      anchor: current.anchor,
      harness: current.harness,
      id: current.id,
      pid: current.pid,
      lastEvent: current.last_event,
      lastSeen: current.last_seen,
      live: this.rowLiveness(current).live,
    };
  }

  private row(id: string): SessionRow | null {
    const row = this.store.database
      .query<SessionRow, [string]>("SELECT * FROM sessions WHERE id = ?")
      .get(id) ?? null;
    return row ? this.normalizeRow(row) : null;
  }

  private normalizeRow(row: SessionRow): SessionRow {
    return {
      ...row,
      anchor: row.anchor === "ttl" ? "ttl" : "pid",
      scope: row.scope ?? "",
    };
  }

  private downgradeSharedPid(row: SessionRow): SessionRow {
    if (row.anchor !== "pid" || !this.sharedPids().has(row.pid)) return row;
    // A shared pid stops proving life, but a dead pid still proves death, so only
    // a live host process earns the fresh clock the TTL anchor is read against.
    const lastSeen = this.isPidAlive(row.pid) ? new Date().toISOString() : row.last_seen;
    const downgraded = { ...row, anchor: "ttl" as const, last_seen: lastSeen };
    if (this.store.readOnly) return downgraded;
    const result = this.store.database
      .query(
        "UPDATE sessions SET anchor = 'ttl', last_seen = ? WHERE id = ? AND pid = ? AND anchor = 'pid'",
      )
      .run(lastSeen, row.id, row.pid);
    if (result.changes > 0) return downgraded;
    return this.row(row.id) ?? row;
  }

  private sharedPids(): Set<number> {
    if (this.sharedPidCache) return this.sharedPidCache;
    // Liveness runs once per held work row, so this aggregation must stay per process.
    const rows = this.store.database
      .query<{ pid: number }, []>(
        "SELECT pid FROM sessions GROUP BY pid HAVING COUNT(DISTINCT id) > 1",
      )
      .all();
    this.sharedPidCache = new Set(rows.map((row) => row.pid));
    return this.sharedPidCache;
  }

  private rowLiveness(row: SessionRow): SessionLiveness {
    if (row.anchor === "pid") {
      const live = this.isPidAlive(row.pid);
      return {
        live,
        reason: live ? `PID ${row.pid} is alive` : `PID ${row.pid} is no longer alive`,
      };
    }
    const lastSeen = Date.parse(row.last_seen);
    const live = Number.isFinite(lastSeen) && Date.now() - lastSeen <= sessionTtlMs;
    return {
      live,
      reason: live
        ? "TTL session was seen within the 60-minute window"
        : "TTL session last_seen is older than the 60-minute window",
    };
  }

  private recentTtlSession(harness: Harness | null): SessionRow | null {
    const cutoff = new Date(Date.now() - sessionTtlMs).toISOString();
    if (!harness) {
      return this.store.database
        .query<SessionRow, [string, string]>(
          `SELECT * FROM sessions
           WHERE anchor = 'ttl' AND scope = ? AND last_seen >= ?
           ORDER BY last_seen DESC LIMIT 1`,
        )
        .get(this.scope, cutoff) ?? null;
    }
    return this.store.database
      .query<SessionRow, [string, string, Harness, Harness]>(
        `SELECT * FROM sessions
         WHERE anchor = 'ttl' AND scope = ? AND last_seen >= ?
           AND (harness = ? OR harness IS NULL)
         ORDER BY CASE WHEN harness = ? THEN 0 ELSE 1 END, last_seen DESC
         LIMIT 1`,
      )
      .get(this.scope, cutoff, harness, harness) ?? null;
  }

  private guessHarness(): Harness | null {
    if (
      process.env.CODEX_SESSION_ID ||
      process.env.CODEX_THREAD_ID ||
      process.env.CODEX_CI ||
      process.env.CODEX_SHELL
    ) {
      return "codex";
    }
    if (
      process.env.CLAUDE_CODE_SESSION_ID ||
      process.env.CLAUDE_SESSION_ID ||
      Object.keys(process.env).some((name) => name.startsWith("CLAUDE_CODE_"))
    ) {
      return "claude";
    }
    return null;
  }

  private disabled(): boolean {
    return this.store.readOnly || process.env.MAESTRO_SESSION_NONE === "1";
  }

  private ensureColumn(name: string, migration: string): void {
    const hasColumn = () =>
      this.store.database
        .query<{ name: string }, []>("PRAGMA table_info(sessions)")
        .all()
        .some((column) => column.name === name);
    if (hasColumn()) return;
    try {
      this.store.migrate(migration);
    } catch (error) {
      if (!hasColumn()) throw error;
    }
  }

  private isPidAlive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      // EPERM means the process exists but this checker may not signal it
      // (e.g. a sandboxed peer probing a foreign session); only ESRCH is death.
      return (error as NodeJS.ErrnoException).code === "EPERM";
    }
  }

  private hostPid(): { pid: number; verified: boolean } {
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
      if (/(^|\/)(codex|claude|cursor)(\s|$)/i.test(command)) {
        return { pid: candidate, verified: true };
      }
      candidate = Number(match[1]);
    }
    return { pid: directParent, verified: false };
  }
}
