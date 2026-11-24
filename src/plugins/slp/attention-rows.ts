import { existsSync } from "node:fs";
import { readFile, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Store, resolveStoreLocation, tableExists } from "../../kernel/store.ts";
import { compactLine, compactsForPane } from "../session-telemetry.ts";

// Hub d96/d97: one runtime process per generation, opened as the maestro
// plugin's `runtime` pane, holds the only Herdr subscription and resolves
// every event against the store deterministically; no model judges attention
// (A3). It writes the store in-process as the reserved actor `runtime`.

const runtimeActor = "runtime";

export const runtimeEnvironment = {
  generation: "MAESTRO_SLP_GENERATION",
  team: "MAESTRO_SLP_TEAM",
} as const;

export const dedupeWindowMs = 5_000;

// d832: a seat that just pushed a return, accept or note finishes its turn
// well after the push; its idle inside this window is not a second wake.
export const pushWindowMs = 60_000;

// l3: a holder that answers a silence nudge with a record of its own has given
// the store the one thing pane idleness cannot see - why it is waiting. That
// account quiets the same nudge while the record is younger than this, measured
// from the record rather than from the previous store write, so a seat is not
// nudged again at the next idle sample for having reported. It withholds
// without consuming the once-rule mark, so the first idle event that arrives
// once the account is older than this still carries the nudge. That is a
// weaker promise than it sounds: w742 Q1, the runtime has no timer of its own
// and acts only on pane status changes, so a pane that goes quiet and never
// changes status again produces no further event to carry it - as was already
// true of the 60-second window this widens.
export const silenceAnsweredMs = 15 * 60_000;

export const runningWaitMs = 10_000;

export const replayQuietMs = 1_000;

export const replayMaxMs = 60_000;

export const watchLines = 40;

type SlpRole = "lead" | "peer";

export type WorkState = "OPEN" | "ACTIVE" | "RETURNED" | "DONE";

export type StallKind = "silence";

export interface RoleRow {
  name: string;
  pane_id: string;
  role: SlpRole;
  workspace_id: string;
}

export interface TeamRow {
  generation: number;
  project_path: string;
  state: "RUNNING" | "STOPPED";
  supervisor_pane_id: string;
  team_id: string;
  workspace_id: string;
}

// The Herdr agent name a Hub Supervisor pane falls back to when team start
// ran outside a pane and recorded none.
export const hubAgentName = "supervisor";

export interface WorkRow {
  assigned_to: string;
  created_by: string;
  id: string;
  owner: string | null;
  return_revision: number;
  state: WorkState;
}

interface EntryRow {
  actor: string;
  flag: string | null;
  id: number;
  kind: string;
}

interface RecordRow {
  actor: string;
  at: string;
  flag: string | null;
  label: string;
  mark: string;
}

export interface PendingLine {
  line: string;
  queuedAt: string;
  // w708 (d867): a stall line is about one item in one state, so it carries that
  // state and holder and is revalidated against the store before it is sent.
  stall?: { holder: string; state: WorkState; workId: string };
  subject: string;
}

interface UnreachableLine {
  droppedAt: string;
  line: string;
  reason: string;
  subject: string;
}

export interface RuntimeState {
  // Last successful delivery per target: the turn it causes is the runtime's
  // own prompt, not the seat finishing its work (same reading as F15).
  delivered: Record<string, number>;
  idleWakes: Record<string, number>;
  pending: Record<string, PendingLine[]>;
  recent: Record<string, number>;
  // w708 (d867): the mark is the holder's latest record ("entry:12", "decision:d5"
  // or "none"), not an entry id, so supervision from above no longer re-arms it.
  stalls: Record<string, string>;
  // Live row 17 (g18): a wake with no resolvable target is dropped once with
  // its reason, never queued for a pane that does not exist.
  unreachable: UnreachableLine[];
}

export interface SlpRuntimeConfig {
  environment?: Record<string, string | undefined>;
  generation: number;
  projectPath: string;
  teamId: string;
}

export function emptyState(): RuntimeState {
  return { delivered: {}, idleWakes: {}, pending: {}, recent: {}, stalls: {}, unreachable: [] };
}

function statePath(directory: string): string {
  return join(directory, "state.json");
}

export async function readRuntimeState(directory: string): Promise<RuntimeState | null> {
  const path = statePath(directory);
  if (!existsSync(path)) return null;
  try {
    const parsed = JSON.parse(await readFile(path, "utf8")) as Partial<RuntimeState>;
    const state = { ...emptyState(), ...parsed };
    // A state file written before w708 holds entry ids as numbers; they mean the
    // same thing in the old vocabulary, so they are read as entry marks.
    state.stalls = Object.fromEntries(
      Object.entries(state.stalls).map(([key, mark]) => [key, typeof mark === "number" ? `entry:${mark}` : mark]),
    );
    return state;
  } catch {
    return null;
  }
}

export async function writeRuntimeState(directory: string, state: RuntimeState): Promise<void> {
  const pending = `${statePath(directory)}.${process.pid}.tmp`;
  await writeFile(pending, `${JSON.stringify(state, null, 2)}\n`);
  await rename(pending, statePath(directory));
}

export async function runtimeLockHolder(directory: string): Promise<number | null> {
  const holder = Number((await readFile(join(directory, "runtime.lock"), "utf8").catch(() => "")).trim());
  if (!Number.isInteger(holder) || holder <= 0) return null;
  try {
    process.kill(holder, 0);
    return holder;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM" ? holder : null;
  }
}

export function openStore(projectPath: string, readonly = false): Store {
  return new Store(resolveStoreLocation(projectPath).path, { readonly });
}

export function teamRow(store: Store, teamId: string, generation: number): TeamRow | null {
  if (!tableExists(store, "slp_local_teams")) return null;
  const supervisorPane = store.hasColumn("slp_local_teams", "supervisor_pane_id") ? "supervisor_pane_id" : "'' AS supervisor_pane_id";
  return store.database
    .query<TeamRow, [string, number]>(
      `SELECT team_id, generation, project_path, state, workspace_id, ${supervisorPane}
       FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
    )
    .get(teamId, generation) ?? null;
}

export function roleRows(store: Store, teamId: string, generation: number): RoleRow[] {
  return store.database
    .query<RoleRow, [string, number]>(
      `SELECT name, role, pane_id, workspace_id FROM slp_local_roles
       WHERE team_id = ? AND generation = ?
       ORDER BY CASE role WHEN 'lead' THEN 0 ELSE 1 END, name`,
    )
    .all(teamId, generation);
}

export function stopInProgress(store: Store, teamId: string, generation: number): boolean {
  return store.database
    .query<{ present: number }, [string, number]>(
      `SELECT 1 AS present FROM slp_lifecycle_operations
       WHERE team_id = ? AND generation = ? AND operation = 'STOP'`,
    )
    .get(teamId, generation) !== null;
}

export function teamCard(store: Store, teamId: string, generation: number): WorkRow | null {
  return store.database
    .query<WorkRow, [string, number]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? ORDER BY created_at, id LIMIT 1`,
    )
    .get(teamId, generation) ?? null;
}

export function heldActive(
  store: Store,
  teamId: string,
  generation: number,
  name: string,
  excludeId: string | null = null,
): WorkRow | null {
  return store.database
    .query<WorkRow, [string, number, string, string]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? AND state = 'ACTIVE' AND owner = ? AND id <> ?
       ORDER BY updated_at DESC, id LIMIT 1`,
    )
    .get(teamId, generation, name, excludeId ?? "") ?? null;
}

// w708 (d867): a record against an item is an entry OR a decision. The detector
// read slp_work_entries alone, so `maestro decide --work <id>` - the practice this
// team runs on - left no trace it could see, and it called a seat with no entries
// silent while that seat had recorded a decision sixteen minutes earlier. `actor`
// narrows the read to one seat, which is how the once-rule keys on the HOLDER's
// own records rather than on anyone's.
// Two questions are about entries alone and keep this narrow read: whether the
// last ENTRY was already a pane:lost mark, and whether the HOLDER's own latest
// ENTRY declared --blocked. Both ask after a flag, and a flag is written by one
// seat into one table: a decision carries no flag at all, and another seat's
// entry is not this seat's declaration, so neither may mask one.
export function latestEntry(store: Store, workId: string, actor?: string): EntryRow | null {
  const sql = `SELECT id, kind, actor, flag FROM slp_work_entries
       WHERE work_id = ?${actor ? " AND actor = ?" : ""} ORDER BY id DESC LIMIT 1`;
  return (actor
    ? store.database.query<EntryRow, [string, string]>(sql).get(workId, actor)
    : store.database.query<EntryRow, [string]>(sql).get(workId)) ?? null;
}

export function heldById(store: Store, workId: string): WorkRow | null {
  return store.database
    .query<WorkRow, [string]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work WHERE id = ?`,
    )
    .get(workId) ?? null;
}

export function latestRecord(store: Store, workId: string, actor?: string): RecordRow | null {
  const entry = store.database
    .query<EntryRow & { created_at: string }, [string]>(
      `SELECT id, kind, actor, flag, created_at FROM slp_work_entries
       WHERE work_id = ?${actor ? " AND actor = ?" : ""} ORDER BY id DESC LIMIT 1`,
    )
    .get(...(actor ? [workId, actor] : [workId]) as [string]) ?? null;
  const decision = tableExists(store, "slp_decisions")
    ? store.database
      .query<{ actor: string; created_at: string; id: string }, [string]>(
        `SELECT id, actor, created_at FROM slp_decisions
         WHERE work_id = ?${actor ? " AND actor = ?" : ""} ORDER BY created_at DESC, id DESC LIMIT 1`,
      )
      .get(...(actor ? [workId, actor] : [workId]) as [string]) ?? null
    : null;
  const fromEntry: RecordRow | null = entry
    ? { actor: entry.actor, at: entry.created_at, flag: entry.flag, label: `entry ${entry.kind}`, mark: `entry:${entry.id}` }
    : null;
  const fromDecision: RecordRow | null = decision
    ? { actor: decision.actor, at: decision.created_at, flag: null, label: `decision ${decision.id}`, mark: `decision:${decision.id}` }
    : null;
  if (!fromEntry) return fromDecision;
  if (!fromDecision) return fromEntry;
  return fromDecision.at > fromEntry.at ? fromDecision : fromEntry;
}

// l3: the runtime's own stall entry is the mark of a nudge already delivered, so
// a holder record written after it is an account of an item the holder knows is
// being watched, rather than a fresh store write that should re-arm the alarm.
// w743 F6: the test is deliberately this wide and not "the record that answered
// the first nudge". Once an item has been nudged for silence, every later record
// its holder writes carries the longer window, so the seat stays quiet by going
// on reporting - which is l3's whole complaint, that reporting made the runtime
// louder. The stall entry names no holder, so a different seat holding the item
// later inherits the longer window too; that is a wider quiet on an item already
// nudged once, never a nudge nobody gets.
export function answersStall(store: Store, workId: string, kind: StallKind, record: RecordRow): boolean {
  const sent = store.database
    .query<{ created_at: string }, [string, string, string]>(
      `SELECT created_at FROM slp_work_entries
       WHERE work_id = ? AND actor = ? AND flag = ? ORDER BY id DESC LIMIT 1`,
    )
    .get(workId, runtimeActor, `stall:${kind}`) ?? null;
  return sent !== null && record.at > sent.created_at;
}

// The evidence names the latest record whoever wrote it, so a reader can tell a
// seat that has been recording from one that truly has not.
export function recordEvidence(record: RecordRow | null, separator: string): string {
  return record ? `${separator} latest ${record.label} by ${record.actor}` : `${separator} no entries or decisions`;
}

// room d137: the compact count rides the evidence of a nudge the runtime was
// already sending, and nothing else. It is not a nudge kind, it fires no
// attention of its own, and an empty string here leaves the old line intact.
export function compactEvidence(store: Store, paneId: string): string {
  const line = compactLine(compactsForPane(store, paneId));
  return line === "" ? "" : `; ${line}`;
}

export function activityMark(store: Store, teamId: string, generation: number): number {
  return store.database
    .query<{ mark: number | null }, [string, number]>(
      `SELECT MAX(id) AS mark FROM slp_activity
       WHERE team_id = ? AND generation = ? AND actor <> '${runtimeActor}'`,
    )
    .get(teamId, generation)?.mark ?? 0;
}

// Advisor F15: a work return, accept or note already woke the counterpart
// through the d753 push; the seat's own idle right after is not a second wake.
export function pushedRecently(store: Store, teamId: string, generation: number, name: string, now: number): boolean {
  const since = new Date(now - pushWindowMs).toISOString();
  return store.database
    .query<{ present: number }, [string, number, string, string]>(
      `SELECT 1 AS present FROM slp_activity
       WHERE team_id = ? AND generation = ? AND actor = ?
         AND operation IN ('work.return', 'work.accept', 'work.note') AND created_at >= ?
       LIMIT 1`,
    )
    .get(teamId, generation, name, since) !== null;
}

function reworkGrantOpen(store: Store, work: WorkRow): boolean {
  if (work.state !== "RETURNED" || !tableExists(store, "slp_rework_grants")) return false;
  return store.database
    .query<{ present: number }, [string, number]>(
      `SELECT 1 AS present FROM slp_rework_grants
       WHERE work_id = ? AND return_revision = ? AND consumed_at IS NULL`,
    )
    .get(work.id, work.return_revision) !== null;
}

// The same rule maestro status prints as `*` (d758, slp-v2 nextStep): the
// item waits on nobody else, so the seat may act on it.
function waitsOnSeat(seat: RoleRow, roles: RoleRow[], work: WorkRow, grantOpen: boolean): boolean {
  const assigneeRole = roles.find((role) => role.name === work.assigned_to)?.role ?? "peer";
  const reviewing = assigneeRole !== "lead" && seat.role === "lead" && seat.name !== work.assigned_to;
  const mine = work.assigned_to === seat.name;
  switch (work.state) {
    case "OPEN":
      return mine;
    case "ACTIVE":
      return work.owner === seat.name;
    case "RETURNED":
      return grantOpen ? mine : reviewing;
    default:
      return false;
  }
}

export function seatHasStar(store: Store, team: TeamRow, seat: RoleRow, roles: RoleRow[]): boolean {
  const items = store.database
    .query<WorkRow, [string, number]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? AND state <> 'DONE'`,
    )
    .all(team.team_id, team.generation)
    .filter((work) => {
      if (seat.role === "peer") return work.assigned_to === seat.name;
      return work.assigned_to === seat.name || work.created_by === seat.name;
    });
  return items.some((work) => waitsOnSeat(seat, roles, work, reworkGrantOpen(store, work)));
}

export function recordEntry(
  store: Store,
  team: TeamRow,
  workId: string,
  body: string,
  flag: string,
  operation: string,
): number {
  const now = new Date().toISOString();
  store.database.exec("BEGIN IMMEDIATE");
  try {
    const inserted = store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, flag, created_at)
         VALUES (?, 'NOTE', ?, ?, ?, ?)`,
      )
      .run(workId, runtimeActor, body, flag, now);
    store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, ?, ?, 'work', ?, ?)`,
      )
      .run(team.team_id, team.generation, runtimeActor, operation, workId, now);
    store.database.exec("COMMIT");
    return Number(inserted.lastInsertRowid);
  } catch (error) {
    try {
      store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
}

export type WakeTarget = { target: string } | { reason: string; target: null };

export function roleTarget(roles: RoleRow[], role: SlpRole): WakeTarget {
  const name = roles.find((candidate) => candidate.role === role)?.name;
  return name ? { target: name } : { reason: `the team has no ${role}`, target: null };
}

// d763: the fixed template, never code advice; the actor is now the runtime.
export function stallLine(workId: string, kind: StallKind, evidence: string): string {
  return `[from ${runtimeActor}][${workId}] ${kind} ${evidence}; stop and run: maestro work note ${workId} "<what you need>" --blocked`;
}

// w708 (d867): the copy is information about someone else's item, so it ends
// where the d753 push lines end - at a read. It carried the holder's imperative
// verbatim, and a reviewing seat came within one step of recording a false
// blocker on an ACTIVE item it does not hold.
export function stallCopyLine(workId: string, kind: StallKind, evidence: string, holder: string): string {
  return `[from ${runtimeActor}][${workId} copy] ${kind} ${evidence}; ${holder} holds ${workId} and has been nudged; read: maestro status ${workId}`;
}
