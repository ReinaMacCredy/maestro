import { CliError } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, tableExists } from "../../kernel/store.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import type { SlpRole } from "./runtime-plan.ts";
import { currentPaneId } from "../session-telemetry.ts";
import { canonicalCheckoutRoot, type WorkState } from "./store.ts";

export interface ActiveLocalTeam {
  configuration_json: string;
  generation: number;
  pack_digest: string;
  project_path: string;
  room_store_path: string;
  runtime_pane_id: string;
  supervisor_pane_id: string;
  team_id: string;
  workspace_id: string;
}

// The Hub Supervisor's pane recorded at team start; the Herdr agent name
// `supervisor` is only the fallback for a start that ran outside a pane.
export function hubSupervisorTarget(team: Pick<ActiveLocalTeam, "supervisor_pane_id">): string {
  return team.supervisor_pane_id || "supervisor";
}

export interface SlpActor {
  name: string;
  role: SlpRole;
  team: ActiveLocalTeam;
}

// room d117: the reviewer of the Lead's work is the Hub Supervisor, which
// holds no seat in the team and so is not an SlpActor. The reviewer
// operations - accept, rework, and the status read behind them - are widened
// to this type, which every SlpActor already satisfies; nothing else in the
// protocol accepts it, so a hub actor can never take, return or note work.
export interface SlpReviewerActor {
  name: string;
  role: SlpRole | typeof hubReviewerName;
  team: ActiveLocalTeam;
}

export interface SlpWorkRow {
  acceptance_outcome: string | null;
  accepted_by: string | null;
  abandoned_at: string | null;
  abandoned_by: string | null;
  abandonment_reason: string | null;
  assigned_to: string;
  created_at: string;
  created_by: string;
  current_return: string | null;
  generation: number;
  id: string;
  objective: string;
  owner: string | null;
  return_revision: number;
  state: WorkState;
  team_id: string;
  updated_at: string;
}

export function activeLocalTeam(context: PluginContext): ActiveLocalTeam | null {
  if (!tableExists(context.store, "slp_local_teams")) return null;
  return context.store.database
    .query<ActiveLocalTeam, [string]>(
      `SELECT team_id, generation, room_store_path, project_path,
              configuration_json, pack_digest, workspace_id, runtime_pane_id,
              supervisor_pane_id
       FROM slp_local_teams
       WHERE state = 'RUNNING' AND project_path = ?
       ORDER BY generation DESC LIMIT 1`,
    )
    .get(canonicalCheckoutRoot(process.cwd())) ?? null;
}

function hasAdoptedLocalSlp(context: PluginContext): boolean {
  return tableExists(context.store, "slp_local_teams") &&
    context.store.database
      .query<{ present: number }, [string]>(
        "SELECT 1 AS present FROM slp_local_teams WHERE project_path = ? LIMIT 1",
      )
      .get(canonicalCheckoutRoot(process.cwd()))?.present === 1;
}

export function requireActiveOrLegacy(context: PluginContext): ActiveLocalTeam | null {
  const active = activeLocalTeam(context);
  if (!active && hasAdoptedLocalSlp(context)) {
    throw new CliError("NO_ACTIVE_TEAM", "no running SLP team is bound to this workspace");
  }
  return active;
}

export function requireSlpActor(context: PluginContext, allowed: readonly SlpRole[]): SlpActor {
  const team = activeLocalTeam(context);
  if (!team) throw new CliError("NO_ACTIVE_TEAM", "no running SLP team is bound to this workspace");
  const paneId = currentPaneId();
  if (!paneId) {
    throw new CliError(
      "ROLE_UNPROVEN",
      "SLP role authority requires the current Herdr pane identity",
    );
  }
  const role = context.store.database
    .query<{ name: string; role: SlpRole }, [string, number, string]>(
      `SELECT name, role FROM slp_local_roles
       WHERE team_id = ? AND generation = ? AND pane_id = ?`,
    )
    .get(team.team_id, team.generation, paneId);
  if (!role) {
    throw new CliError(
      "ROLE_UNPROVEN",
      `pane ${paneId} is not a role in ${team.team_id}:g${team.generation}`,
    );
  }
  if (!allowed.includes(role.role)) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      `${role.role} cannot perform this operation`,
      { actor: role.name, allowed, role: role.role },
    );
  }
  return { ...role, team };
}

export function requireSlpWork(context: PluginContext, actor: SlpReviewerActor, id: string): SlpWorkRow {
  const work = context.store.database
    .query<SlpWorkRow, [string, string, number]>(
      "SELECT * FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
    )
    .get(id, actor.team.team_id, actor.team.generation);
  if (!work) throw new CliError("NOT_FOUND", `SLP work not found: ${id}`);
  return work;
}

export function requireRunningState(store: Store, team: ActiveLocalTeam): void {
  const state = store.database
    .query<{ state: string }, [string, number]>(
      "SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?",
    )
    .get(team.team_id, team.generation)?.state;
  if (state !== "RUNNING") {
    throw new CliError("NO_ACTIVE_TEAM", `no running SLP team in ${team.project_path}`);
  }
}

export function requireRunningGeneration(store: Store, team: ActiveLocalTeam): void {
  requireRunningState(store, team);
  if (tableExists(store, "slp_lifecycle_operations")) {
    const stopping = store.database
      .query<{ present: number }, [string, number]>(
        `SELECT 1 AS present FROM slp_lifecycle_operations
         WHERE team_id = ? AND generation = ? AND operation = 'STOP'
           AND phase <> 'COMMITTED'`,
      )
      .get(team.team_id, team.generation);
    if (stopping) {
      throw new CliError(
        "TEAM_STOP_IN_PROGRESS",
        `${team.team_id}:g${team.generation} is shutting down; retry after team stop finishes`,
      );
    }
  }
}

export function workData(work: SlpWorkRow): Record<string, unknown> {
  return {
    acceptanceOutcome: work.acceptance_outcome,
    acceptedBy: work.accepted_by,
    abandonedAt: work.abandoned_at,
    abandonedBy: work.abandoned_by,
    abandonmentReason: work.abandonment_reason,
    assignedTo: work.assigned_to,
    createdAt: work.created_at,
    createdBy: work.created_by,
    currentReturn: work.current_return,
    generation: work.generation,
    id: work.id,
    objective: work.objective,
    owner: work.owner,
    returnRevision: work.return_revision,
    state: work.state,
    teamId: work.team_id,
    updatedAt: work.updated_at,
  };
}

// room d117: the reviewer of a Lead's work is the Hub Supervisor; a Peer's
// reviewer is the Lead.
type WorkReviewer =
  | { kind: "hub" }
  | { kind: "role"; role: "lead" };

export const hubReviewerName = "hub-supervisor";

function reviewerLabel(reviewer: WorkReviewer): string {
  return reviewer.kind === "hub" ? hubReviewerName : reviewer.role;
}

export function expectedReviewer(
  context: PluginContext,
  actor: SlpReviewerActor,
  work: SlpWorkRow,
): WorkReviewer {
  const assignee = context.store.database
    .query<{ role: SlpRole }, [string, number, string]>(
      `SELECT role FROM slp_local_roles
       WHERE team_id = ? AND generation = ? AND name = ?`,
    )
    .get(actor.team.team_id, actor.team.generation, work.assigned_to);
  if (!assignee) {
    throw new CliError(
      "SLP_BINDING_MISSING",
      `assignee role is missing for ${work.assigned_to}`,
    );
  }
  return assignee.role === "lead" ? { kind: "hub" } : { kind: "role", role: "lead" };
}

export function requireWorkReviewer(
  context: PluginContext,
  actor: SlpReviewerActor,
  work: SlpWorkRow,
  operation: "accept" | "grant rework",
): void {
  const reviewer = expectedReviewer(context, actor, work);
  // The Hub reviews the Lead and nothing below it; a hub actor reaching for a
  // Peer's item is refused just as firmly as a seat reaching for the Lead's.
  if ((reviewer.kind === "hub") !== (actor.role === hubReviewerName)) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      reviewer.kind === "hub"
        ? `${actor.role} cannot ${operation} for work assigned to ${work.assigned_to};` +
          ` the Hub Supervisor reviews the Lead's work from ~/maestro`
        : `the Hub Supervisor cannot ${operation} for work assigned to ${work.assigned_to};` +
          ` its reviewer is the ${reviewer.role} (d72)`,
      { actor: actor.name, expectedReviewer: reviewerLabel(reviewer) },
    );
  }
  if (reviewer.kind === "hub") return;
  if (actor.role !== reviewer.role || actor.name === work.assigned_to) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      `${actor.role} cannot ${operation} for work assigned to ${work.assigned_to}`,
      { actor: actor.name, expectedReviewer: reviewer.role },
    );
  }
}

export function noticeSummary(body: string): string {
  const line = body.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > 160 ? `${line.slice(0, 157)}...` : line;
}

// d753/d760: the store is the truth; the pushed line is only the wake-up.
export async function pushNotice(
  projectPath: string,
  fromRole: SlpRole | "hub-supervisor",
  target: string | null,
  subject: string,
  summary: string,
  read: string,
): Promise<void> {
  await pushLine(
    projectPath,
    target,
    subject,
    `[from ${fromRole}][${subject}] ${noticeSummary(summary)}; read: ${read}`,
  );
}

async function pushLine(
  projectPath: string,
  target: string | null,
  subject: string,
  line: string,
): Promise<void> {
  if (!target) {
    process.stderr.write(`warning: no pane to notify about ${subject}; the store remains the truth\n`);
    return;
  }
  try {
    await new HerdrSlpRuntime().notify(projectPath, target, line);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(
      `warning: could not notify ${target} about ${subject}: ${message}; the store remains the truth\n`,
    );
  }
}

export function rolePaneName(
  context: PluginContext,
  actor: Pick<SlpReviewerActor, "team">,
  role: SlpRole,
): string | null {
  return context.store.database
    .query<{ name: string }, [string, number, string]>(
      `SELECT name FROM slp_local_roles
       WHERE team_id = ? AND generation = ? AND role = ?
       ORDER BY created_at LIMIT 1`,
    )
    .get(actor.team.team_id, actor.team.generation, role)?.name ?? null;
}

// room d117: the RETURNED push follows the reviewer. For the Lead's work that
// is the Hub Supervisor's pane, recorded at team start, which is the same
// target work accept already uses to report a hub-created item back to the
// room.
export function reviewerPushTarget(
  context: PluginContext,
  actor: SlpActor,
  work: SlpWorkRow,
): string | null {
  const reviewer = expectedReviewer(context, actor, work);
  return reviewer.kind === "hub"
    ? hubSupervisorTarget(actor.team)
    : rolePaneName(context, actor, reviewer.role);
}

export function readSlpWork(context: PluginContext, actor: SlpReviewerActor, id: string): SlpWorkRow {
  return requireSlpWork(context, actor, id);
}

export function recordProjectActivity(
  context: PluginContext,
  actor: SlpReviewerActor,
  operation: string,
  targetId: string,
  now: string,
): void {
  context.store.database
    .query(
      `INSERT INTO slp_activity
        (team_id, generation, actor, operation, target_type, target_id, created_at)
       VALUES (?, ?, ?, ?, 'work', ?, ?)`,
    )
    .run(actor.team.team_id, actor.team.generation, actor.name, operation, targetId, now);
}

export function requireWorkTransition(changes: number, id: string, operation: string): void {
  if (changes !== 1) {
    throw new CliError(
      "INVALID_STATE",
      `${id} changed before ${operation}; inspect its current status and retry`,
    );
  }
}

export function roleRows(store: Store, teamId: string, generation: number, local: boolean) {
  const table = local ? "slp_local_roles" : "slp_team_roles";
  return store.database
    .query<{
      brief_digest: string;
      instance_id: string;
      name: string;
      pack_digest: string;
      pane_id: string;
      profile: string;
      ready_challenge: string;
      role: SlpRole;
      workspace_id: string;
    }, [string, number]>(
      `SELECT name, pane_id, role, workspace_id, instance_id, pack_digest,
              brief_digest, ready_challenge, profile FROM ${table}
       WHERE team_id = ? AND generation = ?
       ORDER BY CASE role WHEN 'lead' THEN 0 ELSE 1 END, name`,
    )
    .all(teamId, generation);
}

export function clipLine(text: string, limit: number): string {
  const line = text.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > limit ? `${line.slice(0, limit - 3)}...` : line;
}

// room d129 point 4: the capture points are recorded, never inferred (d705).
// A reviewer's rework grant and a decision that both --replaces and names
// --work are the two moments a correction is already being written down; the
// row is the mark, and no text is read to produce it.
export function markLessonCandidate(store: Store, workId: string, source: string, at: string): void {
  if (!tableExists(store, "slp_lesson_candidates")) return;
  store.database
    .query(
      `INSERT OR IGNORE INTO slp_lesson_candidates (work_id, source, recorded_at)
       VALUES (?, ?, ?)`,
    )
    .run(workId, source, at);
}
