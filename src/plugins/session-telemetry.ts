import type { EventLog } from "../kernel/log.ts";
import type { Harness } from "../kernel/sessions.ts";
import { type Store, tableExists } from "../kernel/store.ts";

// room d137: per-pane session telemetry starts with the compact count. One
// event-log row per compaction, keyed by the harness session id and by the
// Herdr pane the hook fired in when the store knows one; the reviewer reads
// the figure back beside the seat. It informs judgment and never gates,
// nudges or stops a seat by itself (d96/d97 hold), so nothing here compares
// the count with a threshold.
const compactEventType = "session.compact";

export interface CompactSummary {
  count: number;
  lastAt: string;
}

export function recordCompaction(
  log: EventLog,
  sessionId: string,
  harness: Harness | null,
  paneId: string | null,
): void {
  log.append({
    type: compactEventType,
    entityType: "session",
    entityId: sessionId,
    sessionId,
    payload: { harness, pane: paneId },
  });
}

// A compaction recorded before the pane binding was resolvable still has its
// session row; it simply belongs to no seat, which is the honest reading.
export function compactsForPane(store: Store, paneId: string | null): CompactSummary | null {
  if (!paneId || !tableExists(store, "event_log")) return null;
  const row = store.database
    .query<{ count: number; last: string | null }, [string, string]>(
      `SELECT COUNT(*) AS count, MAX(created_at) AS last FROM event_log
       WHERE type = ? AND json_extract(payload, '$.pane') = ?`,
    )
    .get(compactEventType, paneId);
  if (!row || row.count === 0 || !row.last) return null;
  return { count: row.count, lastAt: row.last };
}

// Empty when the seat has compacted nothing, so a caller appends it blind.
export function compactLine(summary: CompactSummary | null, now = Date.now()): string {
  if (!summary) return "";
  const age = relativeAge(summary.lastAt, now);
  return `compacts: ${summary.count}${age === "" ? "" : ` (last ${age})`}`;
}

// The one age renderer the session surfaces share; `maestro status` adds
// "ago" to it and the compact line reads "(last 5m)".
export function relativeAge(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - Date.parse(iso)) / 1000));
  if (!Number.isFinite(seconds)) return "";
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86_400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86_400)}d`;
}
