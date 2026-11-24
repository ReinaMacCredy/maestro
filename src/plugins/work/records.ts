import type { Database } from "bun:sqlite";
import { CliError } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";

export interface WorkRecord {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done" | "cancelled";
  parentId: string | null;
  acceptance: string | null;
  atomicReason: string | null;
  evidence: string | null;
  candidate: string | null;
  heldBy: string | null;
  reclaimedFrom: string | null;
  reclaimedBy: string | null;
  reclaimReason: string | null;
  cancelReason: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface WorkRow {
  id: string;
  title: string;
  kind: string;
  state: "open" | "active" | "done";
  parent_id: string | null;
  acceptance: string | null;
  atomic_reason: string | null;
  evidence: string | null;
  candidate: string | null;
  held_by: string | null;
  reclaimed_from: string | null;
  reclaimed_by: string | null;
  reclaim_reason: string | null;
  cancelled_at: string | null;
  cancel_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface Checkpoint {
  avoid: string;
  next: string;
  state: string;
}

// The latest `checkpoint:` note per work item (Hub d105/d106): three lines,
// state / next / avoid, written incrementally so a compaction never loses them.
export function latestCheckpoints(
  database: Database,
  workIds: string[],
): Map<string, Checkpoint> {
  if (workIds.length === 0) return new Map();
  const placeholders = workIds.map(() => "?").join(", ");
  const rows = database
    .query<{ text: string; work_id: string }, string[]>(
      `SELECT work_id, text FROM work_notes
       WHERE work_id IN (${placeholders}) AND SUBSTR(text, 1, 11) = 'checkpoint:'
       ORDER BY id DESC`,
    )
    .all(...workIds);
  const checkpoints = new Map<string, Checkpoint>();
  for (const row of rows) {
    if (checkpoints.has(row.work_id)) continue;
    checkpoints.set(row.work_id, parseCheckpoint(row.text));
  }
  return checkpoints;
}

function parseCheckpoint(text: string): Checkpoint {
  const fields: Checkpoint = { avoid: "", next: "", state: "" };
  // Agents typing the doc example in a shell send a literal backslash-n; accept both.
  for (const line of text.slice("checkpoint:".length).split(/\n|\\n/)) {
    const match = line.match(/^\s*(state|next|avoid):\s*(.*)$/);
    if (match) fields[match[1] as keyof Checkpoint] = (match[2] ?? "").trim();
  }
  return fields;
}

export interface WorkService {
  get(id: string): WorkRecord | null;
  list(): WorkRecord[];
  children(id: string): WorkRecord[];
  release(id: string, holder: string, updatedAt: string): boolean;
  snapshot(): WorkRecord[];
}

export interface WorkGateInput {
  children: WorkRecord[];
  sessionId: string;
  work: WorkRecord;
}

export interface WorkAddGateInput {
  kind: string;
  parentId: string | null;
  sessionId: string;
  title: string;
}

export interface GateResult {
  blocked: boolean;
  blockers?: Array<{ id: string; state: string }>;
  command?: string;
  evidence?: string;
  origin?: string;
  reason?: string;
}

export function toWork(row: WorkRow): WorkRecord {
  return {
    id: row.id,
    title: row.title,
    kind: row.kind,
    state: row.cancelled_at ? "cancelled" : row.state,
    parentId: row.parent_id,
    acceptance: row.acceptance,
    atomicReason: row.atomic_reason,
    evidence: row.evidence,
    candidate: row.candidate ?? null,
    heldBy: row.held_by,
    reclaimedFrom: row.reclaimed_from,
    reclaimedBy: row.reclaimed_by,
    reclaimReason: row.reclaim_reason,
    cancelReason: row.cancel_reason,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

export function getWork(context: PluginContext, id: string): WorkRecord | null {
  const row = context.store.database
    .query<WorkRow, [string]>("SELECT * FROM work WHERE id = ?")
    .get(id);
  return row ? expireDeadLease(context, toWork(row)) : null;
}

export function expireDeadLease(context: PluginContext, work: WorkRecord): WorkRecord {
  if (!work.heldBy) return work;
  const previousHolder = work.heldBy;
  const liveness = context.sessions.liveness(previousHolder);
  if (liveness.live) return work;
  const updatedAt = new Date().toISOString();
  const expired = { ...work, state: "open" as const, heldBy: null, updatedAt };
  if (context.store.readOnly) return expired;
  const result = context.store.database
    .query(
      `UPDATE work
       SET state = 'open', held_by = NULL, updated_at = ?
       WHERE id = ? AND held_by = ? AND state = 'active'`,
    )
    .run(updatedAt, work.id, previousHolder);
  if (result.changes === 0) return work;
  context.log.append({
    type: "work.lease.expire",
    entityType: "work",
    entityId: work.id,
    payload: { holder: previousHolder, reason: liveness.reason },
  });
  return expired;
}

export function latestLeaseExpiration(
  context: PluginContext,
  id: string,
): { holder: string; reason: string } | null {
  const event = context.log
    .list("work", id)
    .filter((candidate) =>
      candidate.type === "work.start" ||
      candidate.type === "work.done" ||
      candidate.type === "work.lease.expire"
    )
    .at(-1);
  if (event?.type !== "work.lease.expire") return null;
  const payload = event.payload as { holder?: unknown; reason?: unknown };
  return typeof payload.holder === "string" && typeof payload.reason === "string"
    ? { holder: payload.holder, reason: payload.reason }
    : null;
}

export function requireWork(context: PluginContext, id: string): WorkRecord {
  const work = getWork(context, id);
  if (!work) {
    throw new CliError("NOT_FOUND", `work not found: ${id}; run: maestro work list`, {
      command: "maestro work list",
      id,
    });
  }
  return work;
}

export function blockersFor(context: PluginContext, id: string): WorkRecord[] {
  return context.store.database
    .query<{ id: string }, [string]>(
      `SELECT blocker_id AS id
       FROM work_blockers
       WHERE work_id = ?
       ORDER BY CAST(SUBSTR(blocker_id, 2) AS INTEGER)`,
    )
    .all(id)
    .map((blocker) => requireWork(context, blocker.id));
}

export function unresolvedBlockers(context: PluginContext, id: string): WorkRecord[] {
  return blockersFor(context, id).filter(
    (blocker) => blocker.state !== "done" && blocker.state !== "cancelled",
  );
}

function nextBlockerCommand(
  context: PluginContext,
  blocker: WorkRecord,
  sessionId: string,
): string {
  if (blocker.state === "active") {
    return blocker.heldBy === sessionId
      ? `maestro work done ${blocker.id}`
      : "maestro work list";
  }
  return unresolvedBlockers(context, blocker.id).length === 0
    ? `maestro work start ${blocker.id}`
    : "maestro work list";
}

export function blockerDetails(
  context: PluginContext,
  id: string,
  blockers: WorkRecord[],
  sessionId: string,
): { blockers: string[]; command: string; reason: string } {
  const command = nextBlockerCommand(context, blockers[0] as WorkRecord, sessionId);
  return {
    blockers: blockers.map((blocker) => blocker.id),
    command,
    reason:
      `${id} is blocked by unresolved work: ` +
      `${blockers.map((blocker) => `${blocker.id} [${blocker.state}]`).join(", ")}; ` +
      `run: ${command}`,
  };
}

export function listWork(context: PluginContext): WorkRecord[] {
  return context.store.database
    .query<WorkRow, []>("SELECT * FROM work ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)")
    .all()
    .map(toWork)
    .map((work) => expireDeadLease(context, work));
}

export function blockIfNeeded(result: GateResult): void {
  if (!result.blocked) return;
  const details = gateDetails(result);
  throw new CliError("GATE_BLOCKED", details.reason, {
    origin: details.origin,
    ...(result.blockers ? { blockers: result.blockers } : {}),
    ...(result.command ? { command: result.command } : {}),
  });
}

export function gateDetails(result: GateResult): { origin: string; reason: string } {
  return {
    origin: result.origin ?? "unknown",
    reason: result.reason ?? "gate blocked",
  };
}
