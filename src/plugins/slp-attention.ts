import { existsSync } from "node:fs";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError, type CliResult } from "../kernel/cli.ts";
import { Store, resolveStoreLocation, tableExists } from "../kernel/store.ts";
import {
  HerdrClient,
  SlpRuntimeError,
  type HerdrAgent,
  type HerdrEvent,
  type HerdrEventStream,
  type HerdrSubscription,
} from "./herdr-client.ts";
import { resolveHomeDirectory } from "./home.ts";
import { herdrPluginId } from "./slp-plugin.ts";
import { acquireProcessLock, slpRuntimeDirectory } from "./slp-process.ts";

// Hub d96/d97: one runtime process per generation, opened as the maestro
// plugin's `runtime` pane, holds the only Herdr subscription and resolves
// every event against the store deterministically; no model judges attention
// (A3). It writes the store in-process as the reserved actor `runtime`.

export const runtimeActor = "runtime";
export const runtimeEnvironment = {
  generation: "MAESTRO_SLP_GENERATION",
  team: "MAESTRO_SLP_TEAM",
} as const;

const dedupeWindowMs = 5_000;
// d832: a seat that just pushed a return, accept or note finishes its turn
// well after the push; its idle inside this window is not a second wake.
const pushWindowMs = 60_000;
const runningWaitMs = 10_000;
const watchLines = 40;

type SlpRole = "team-supervisor" | "lead" | "peer";
type WorkState = "OPEN" | "ACTIVE" | "RETURNED" | "DONE";
type StallKind = "dialog" | "silence";

interface RoleRow {
  name: string;
  pane_id: string;
  role: SlpRole;
  workspace_id: string;
}

interface TeamRow {
  generation: number;
  project_path: string;
  state: "RUNNING" | "STOPPED";
  team_id: string;
  workspace_id: string;
}

interface WorkRow {
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

interface PendingLine {
  line: string;
  queuedAt: string;
  subject: string;
}

export interface RuntimeState {
  // Last successful delivery per target: the turn it causes is the runtime's
  // own prompt, not the seat finishing its work (same reading as F15).
  delivered: Record<string, number>;
  idleWakes: Record<string, number>;
  pending: Record<string, PendingLine[]>;
  recent: Record<string, number>;
  stalls: Record<string, number>;
}

export interface SlpRuntimeConfig {
  environment?: Record<string, string | undefined>;
  generation: number;
  projectPath: string;
  teamId: string;
}

function emptyState(): RuntimeState {
  return { delivered: {}, idleWakes: {}, pending: {}, recent: {}, stalls: {} };
}

function statePath(directory: string): string {
  return join(directory, "state.json");
}

export async function readRuntimeState(directory: string): Promise<RuntimeState | null> {
  const path = statePath(directory);
  if (!existsSync(path)) return null;
  try {
    const parsed = JSON.parse(await readFile(path, "utf8")) as Partial<RuntimeState>;
    return { ...emptyState(), ...parsed };
  } catch {
    return null;
  }
}

async function writeRuntimeState(directory: string, state: RuntimeState): Promise<void> {
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

function openStore(projectPath: string, readonly = false): Store {
  return new Store(resolveStoreLocation(projectPath).path, { readonly });
}

function teamRow(store: Store, teamId: string, generation: number): TeamRow | null {
  if (!tableExists(store, "slp_local_teams")) return null;
  return store.database
    .query<TeamRow, [string, number]>(
      `SELECT team_id, generation, project_path, state, workspace_id
       FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
    )
    .get(teamId, generation) ?? null;
}

function roleRows(store: Store, teamId: string, generation: number): RoleRow[] {
  return store.database
    .query<RoleRow, [string, number]>(
      `SELECT name, role, pane_id, workspace_id FROM slp_local_roles
       WHERE team_id = ? AND generation = ?
       ORDER BY CASE role WHEN 'team-supervisor' THEN 0 WHEN 'lead' THEN 1 ELSE 2 END, name`,
    )
    .all(teamId, generation);
}

function stopInProgress(store: Store, teamId: string, generation: number): boolean {
  return store.database
    .query<{ present: number }, [string, number]>(
      `SELECT 1 AS present FROM slp_lifecycle_operations
       WHERE team_id = ? AND generation = ? AND operation = 'STOP'`,
    )
    .get(teamId, generation) !== null;
}

function teamCard(store: Store, teamId: string, generation: number): WorkRow | null {
  return store.database
    .query<WorkRow, [string, number]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? ORDER BY created_at, id LIMIT 1`,
    )
    .get(teamId, generation) ?? null;
}

function heldActive(store: Store, teamId: string, generation: number, name: string): WorkRow | null {
  return store.database
    .query<WorkRow, [string, number, string]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? AND state = 'ACTIVE' AND owner = ?
       ORDER BY updated_at DESC, id LIMIT 1`,
    )
    .get(teamId, generation, name) ?? null;
}

function latestEntry(store: Store, workId: string): EntryRow | null {
  return store.database
    .query<EntryRow, [string]>(
      `SELECT id, kind, actor, flag FROM slp_work_entries WHERE work_id = ? ORDER BY id DESC LIMIT 1`,
    )
    .get(workId) ?? null;
}

function activityMark(store: Store, teamId: string, generation: number): number {
  return store.database
    .query<{ mark: number | null }, [string, number]>(
      `SELECT MAX(id) AS mark FROM slp_activity
       WHERE team_id = ? AND generation = ? AND actor <> '${runtimeActor}'`,
    )
    .get(teamId, generation)?.mark ?? 0;
}

// Advisor F15: a work return, accept or note already woke the counterpart
// through the d753 push; the seat's own idle right after is not a second wake.
function pushedRecently(store: Store, teamId: string, generation: number, name: string, now: number): boolean {
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
  const reviewerRole: SlpRole = assigneeRole === "lead" ? "team-supervisor" : "lead";
  const reviewing = seat.role === reviewerRole && seat.name !== work.assigned_to;
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

function seatHasStar(store: Store, team: TeamRow, seat: RoleRow, roles: RoleRow[]): boolean {
  const items = store.database
    .query<WorkRow, [string, number]>(
      `SELECT id, state, owner, assigned_to, created_by, return_revision FROM slp_work
       WHERE team_id = ? AND generation = ? AND state <> 'DONE'`,
    )
    .all(team.team_id, team.generation)
    .filter((work) => {
      if (seat.role === "team-supervisor") return work.state === "ACTIVE" || work.state === "RETURNED";
      if (seat.role === "peer") return work.assigned_to === seat.name;
      return work.assigned_to === seat.name || work.created_by === seat.name;
    });
  return items.some((work) => waitsOnSeat(seat, roles, work, reworkGrantOpen(store, work)));
}

function recordEntry(
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

function seatAbove(seat: RoleRow, roles: RoleRow[]): string | null {
  if (seat.role === "team-supervisor") return "supervisor";
  const above: SlpRole = seat.role === "lead" ? "team-supervisor" : "lead";
  return roles.find((role) => role.role === above)?.name ?? null;
}

function supervisorTarget(seat: RoleRow, roles: RoleRow[]): string | null {
  if (seat.role === "team-supervisor") return "supervisor";
  return roles.find((role) => role.role === "team-supervisor")?.name ?? null;
}

// d763: the fixed template, never code advice; the actor is now the runtime.
function stallLine(workId: string, kind: StallKind, evidence: string): string {
  return `[from ${runtimeActor}][${workId}] ${kind} ${evidence}; stop and run: maestro work note ${workId} "<what you need>" --blocked`;
}

export class AttentionRuntime {
  readonly directory: string;
  private state: RuntimeState = emptyState();
  private statuses = new Map<string, string>();
  private roles: RoleRow[] = [];
  // Panes where Herdr detected an agent before the seat's role row existed
  // (a Peer being opened by work add --to); their status events are wanted
  // so the row is found on the first event after the acknowledgement.
  private candidates = new Set<string>();
  private readonly client: HerdrClient;

  constructor(
    private readonly config: SlpRuntimeConfig,
    private readonly log: (line: string) => void = (line) => process.stderr.write(`${line}\n`),
  ) {
    this.directory = slpRuntimeDirectory(config.projectPath, config.teamId, config.generation);
    this.client = new HerdrClient(config.environment ?? process.env);
  }

  async load(): Promise<void> {
    await mkdir(this.directory, { recursive: true });
    this.state = (await readRuntimeState(this.directory)) ?? emptyState();
  }

  private async persist(): Promise<void> {
    await writeRuntimeState(this.directory, this.state);
  }

  private team(store: Store): TeamRow | null {
    return teamRow(store, this.config.teamId, this.config.generation);
  }

  refreshRoles(store: Store): boolean {
    const next = roleRows(store, this.config.teamId, this.config.generation);
    const changed = next.map((role) => `${role.name}:${role.pane_id}`).join(",") !==
      this.roles.map((role) => `${role.name}:${role.pane_id}`).join(",");
    this.roles = next;
    return changed;
  }

  subscriptions(): HerdrSubscription[] {
    const panes = new Set([...this.roles.map((role) => role.pane_id), ...this.candidates]);
    return [
      ...[...panes].map((pane_id) => ({ pane_id, type: "pane.agent_status_changed" as const })),
      { type: "pane.agent_detected" },
      { type: "pane.exited" },
      { type: "pane.closed" },
    ];
  }

  async seedStatuses(): Promise<void> {
    let agents: HerdrAgent[] = [];
    try {
      agents = await this.client.agentList();
    } catch {
      return;
    }
    for (const agent of agents) this.statuses.set(agent.pane_id, agent.agent_status);
  }

  private async deliver(target: string | null, subject: string, line: string): Promise<void> {
    if (!target) {
      this.log(`no pane to wake about ${subject}; the store remains the truth`);
      return;
    }
    const queue = this.state.pending[target] ?? [];
    queue.push({ line, queuedAt: new Date().toISOString(), subject });
    this.state.pending[target] = queue;
    await this.flush(target);
  }

  // Queue: a wake for a working target waits for its own idle; a failed
  // prompt stays queued (SPEC item 4).
  async flush(target: string): Promise<void> {
    const queue = this.state.pending[target] ?? [];
    if (queue.length === 0) return;
    const pane = this.roles.find((role) => role.name === target)?.pane_id;
    const status = pane ? this.statuses.get(pane) : undefined;
    if (status === "working") {
      await this.persist();
      return;
    }
    while (queue.length > 0) {
      const next = queue[0] as PendingLine;
      try {
        await this.client.agentPrompt(target, next.line);
        queue.shift();
        this.state.delivered[target] = Date.now();
        this.log(`woke ${target}: ${next.subject}`);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.log(`wake of ${target} about ${next.subject} stays queued: ${message}`);
        break;
      }
    }
    if (queue.length === 0) delete this.state.pending[target];
    await this.persist();
  }

  private deduplicated(paneId: string, status: string, now: number): boolean {
    const key = `${paneId}:${status}`;
    const last = this.state.recent[key];
    for (const [candidate, at] of Object.entries(this.state.recent)) {
      if (now - at > dedupeWindowMs * 12) delete this.state.recent[candidate];
    }
    this.state.recent[key] = now;
    return last !== undefined && now - last < dedupeWindowMs;
  }

  // d763 once-rule: one stall entry and one nudge per (item, kind) until that
  // item's latest entry changes; the runtime's own entry is the mark.
  private async stall(
    store: Store,
    team: TeamRow,
    seat: RoleRow,
    kind: StallKind,
    work: WorkRow,
    evidence: string,
  ): Promise<void> {
    const key = `${work.id}:${kind}`;
    const latest = latestEntry(store, work.id);
    if (latest && this.state.stalls[key] === latest.id) return;
    const body = `${kind}: ${evidence}`;
    const entryId = recordEntry(store, team, work.id, body, `stall:${kind}`, "work.stall");
    this.state.stalls[key] = entryId;
    const line = stallLine(work.id, kind, evidence);
    await this.deliver(seat.name, `${work.id} ${kind}`, line);
    const copy = supervisorTarget(seat, this.roles);
    if (copy && copy !== seat.name) await this.deliver(copy, `${work.id} ${kind} copy`, line);
  }

  // Returns true when the subscription must be rebuilt (a new pane to watch).
  async handle(event: HerdrEvent): Promise<boolean> {
    const store = openStore(this.config.projectPath);
    try {
      const team = this.team(store);
      if (!team || team.state !== "RUNNING") return false;
      const paneId = typeof event.data.pane_id === "string" ? event.data.pane_id : null;
      if (!paneId) return false;
      if (event.event === "pane_agent_detected") {
        const changed = this.refreshRoles(store);
        const inTeam = typeof event.data.workspace_id !== "string" || event.data.workspace_id === team.workspace_id;
        const known = this.roles.some((role) => role.pane_id === paneId) || this.candidates.has(paneId);
        if (inTeam && !known) this.candidates.add(paneId);
        return changed || (inTeam && !known);
      }
      let seat = this.roles.find((role) => role.pane_id === paneId);
      let changed = false;
      if (!seat) {
        changed = this.refreshRoles(store);
        seat = this.roles.find((role) => role.pane_id === paneId);
      }
      if (seat) this.candidates.delete(paneId);
      if (!seat) return changed;
      const now = Date.now();
      if (event.event === "pane_exited" || event.event === "pane_closed") {
        if (stopInProgress(store, team.team_id, team.generation)) return changed;
        const loss = event.event === "pane_exited" ? "exited" : "closed";
        if (this.deduplicated(paneId, loss, now)) return changed;
        this.statuses.delete(paneId);
        const card = teamCard(store, team.team_id, team.generation);
        const evidence = `${loss}: ${seat.name} pane ${paneId} (${event.event}); store: ${seat.role} of ${team.team_id} g${team.generation}`;
        if (card) recordEntry(store, team, card.id, evidence, `pane:${loss}`, "pane.lost");
        await this.deliver(supervisorTarget(seat, this.roles), `${seat.name} pane ${loss}`, `[attention] ${seat.name} pane ${loss}`);
        return changed;
      }
      if (event.event !== "pane_agent_status_changed") return changed;
      const status = typeof event.data.agent_status === "string" ? event.data.agent_status : "unknown";
      this.statuses.set(paneId, status);
      if (this.deduplicated(paneId, status, now)) {
        await this.persist();
        return changed;
      }
      if (status === "blocked") {
        const held = heldActive(store, team.team_id, team.generation, seat.name) ??
          teamCard(store, team.team_id, team.generation);
        if (!held) return changed;
        const latest = latestEntry(store, held.id);
        const evidence = `${seat.name} pane ${paneId} agent_status blocked; store: ${held.id} ${held.state}` +
          `${held.owner ? ` owned by ${held.owner}` : ` assigned to ${held.assigned_to}`}` +
          `${latest ? `, latest entry ${latest.kind} by ${latest.actor}` : ", no entries"}`;
        await this.stall(store, team, seat, "dialog", held, evidence);
        return changed;
      }
      if (status !== "idle" && status !== "done") {
        await this.persist();
        return changed;
      }
      const held = heldActive(store, team.team_id, team.generation, seat.name);
      if (held) {
        const latest = latestEntry(store, held.id);
        // d761 layer one: a seat that declared --blocked is legitimately waiting.
        if (latest?.flag === "blocked") {
          await this.flush(seat.name);
          return changed;
        }
        const evidence = `${seat.name} pane ${paneId} agent_status ${status} while ${held.id} ACTIVE owned by ${seat.name}` +
          `${latest ? `; latest entry ${latest.kind} by ${latest.actor}` : "; no entries"}`;
        await this.stall(store, team, seat, "silence", held, evidence);
        await this.flush(seat.name);
        return changed;
      }
      await this.flush(seat.name);
      if (seatHasStar(store, team, seat, this.roles)) return changed;
      if (pushedRecently(store, team.team_id, team.generation, seat.name, now)) return changed;
      const delivered = this.state.delivered[seat.name];
      if (delivered !== undefined && now - delivered < pushWindowMs) return changed;
      // d832: one idle wake per pane until the team's activity log advances.
      const mark = activityMark(store, team.team_id, team.generation);
      if (this.state.idleWakes[paneId] === mark) return changed;
      this.state.idleWakes[paneId] = mark;
      await this.deliver(seatAbove(seat, this.roles), `${seat.name} idle`, `[attention] ${seat.name} idle`);
      return changed;
    } finally {
      store.close();
    }
  }

  async render(): Promise<string> {
    const sections = [`SLP runtime ${this.config.teamId}:g${this.config.generation}`];
    for (const role of this.roles) {
      const status = this.statuses.get(role.pane_id) ?? "unknown";
      let output = "[unavailable]";
      try {
        output = (await this.client.agentRead(role.name, "recent_unwrapped", watchLines)).trimEnd() || "[no output]";
      } catch {}
      sections.push(`=== ${role.name} [${role.role}] ${status} ===\n${output}`);
    }
    const pending = Object.entries(this.state.pending).flatMap(([target, lines]) =>
      lines.map((line) => `  ${target}: ${line.line}`)
    );
    if (pending.length > 0) sections.push(`pending wakes:\n${pending.join("\n")}`);
    return `${sections.join("\n\n")}\n`;
  }
}

export async function runSlpRuntime(config: SlpRuntimeConfig): Promise<number> {
  const runtime = new AttentionRuntime(config);
  await runtime.load();
  const lockPath = join(runtime.directory, "runtime.lock");
  await acquireProcessLock(lockPath, "SLP runtime");
  let stopping = false;
  let stream: HerdrEventStream | null = null;
  const stop = () => {
    stopping = true;
    stream?.close();
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
  process.on("SIGHUP", stop);
  try {
    const deadline = Date.now() + runningWaitMs;
    while (true) {
      const store = openStore(config.projectPath, true);
      let team: TeamRow | null;
      try {
        team = teamRow(store, config.teamId, config.generation);
        if (team?.state === "RUNNING") runtime.refreshRoles(store);
      } finally {
        store.close();
      }
      if (team?.state === "RUNNING") break;
      if (team?.state === "STOPPED") return 0;
      if (Date.now() >= deadline) {
        process.stderr.write(`maestro slp runtime: no running ${config.teamId}:g${config.generation} in ${config.projectPath}\n`);
        return 1;
      }
      await Bun.sleep(100);
    }
    await runtime.seedStatuses();
    const client = new HerdrClient(config.environment ?? process.env);
    while (!stopping) {
      let resubscribe = false;
      stream = await client.subscribe(runtime.subscriptions());
      process.stdout.write(`\u001b[2J\u001b[H${await runtime.render()}`);
      try {
        for await (const event of stream.events) {
          try {
            resubscribe = await runtime.handle(event);
          } catch (error) {
            process.stderr.write(
              `maestro slp runtime: ${error instanceof Error ? error.message : String(error)}\n`,
            );
          }
          process.stdout.write(`\u001b[2J\u001b[H${await runtime.render()}`);
          if (resubscribe) break;
        }
      } finally {
        stream.close();
      }
      if (stopping) return 0;
      if (!resubscribe) {
        process.stderr.write("maestro slp runtime: Herdr closed the subscription; the plugin startup hook reopens this pane after a restart\n");
        return 1;
      }
    }
    return 0;
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
    process.off("SIGHUP", stop);
    const holder = Number((await readFile(lockPath, "utf8").catch(() => "")).trim());
    if (holder === process.pid) await rm(lockPath, { force: true });
  }
}

export function runtimeConfigFromEnvironment(
  environment: Record<string, string | undefined> = process.env,
  cwd = process.cwd(),
): SlpRuntimeConfig {
  const teamId = environment[runtimeEnvironment.team];
  const generation = Number(environment[runtimeEnvironment.generation]);
  if (!teamId || !/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(teamId)) {
    throw new CliError("INVALID_OPTION", `${runtimeEnvironment.team} must name a normalized team id`);
  }
  if (!Number.isInteger(generation) || generation < 1) {
    throw new CliError("INVALID_OPTION", `${runtimeEnvironment.generation} must be a positive integer`);
  }
  return { environment, generation, projectPath: resolve(cwd), teamId };
}

// Every project the Hub registry lists; the plugin hooks run from the plugin
// directory and find running generations through it.
export async function registeredProjects(home = resolveHomeDirectory()): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter((line) => line.trim() !== "");
}

interface RunningGeneration {
  projectPath: string;
  roles: RoleRow[];
  team: TeamRow;
}

export function runningGenerations(projectPath: string): RunningGeneration[] {
  const location = resolveStoreLocation(projectPath);
  if (!existsSync(location.path)) return [];
  const store = new Store(location.path, { readonly: true });
  try {
    if (!tableExists(store, "slp_local_teams") || !tableExists(store, "slp_local_roles")) return [];
    return store.database
      .query<TeamRow, []>(
        `SELECT team_id, generation, project_path, state, workspace_id
         FROM slp_local_teams WHERE state = 'RUNNING' ORDER BY team_id, generation`,
      )
      .all()
      .map((team) => ({
        projectPath: team.project_path,
        roles: roleRows(store, team.team_id, team.generation),
        team,
      }));
  } finally {
    store.close();
  }
}

export interface RuntimePaneOpening {
  paneId: string | null;
}

// Item 3: the pane command is static; team and generation arrive as env
// (advisor F17), cwd is the project so the store resolves like any command.
export async function openRuntimePane(
  client: HerdrClient,
  team: Pick<TeamRow, "generation" | "project_path" | "team_id" | "workspace_id">,
  anchorPaneId: string | null,
): Promise<string> {
  const pane = await client.pluginPaneOpen({
    cwd: team.project_path,
    entrypoint: "runtime",
    env: {
      [runtimeEnvironment.generation]: String(team.generation),
      [runtimeEnvironment.team]: team.team_id,
    },
    placement: anchorPaneId ? "split" : "tab",
    plugin_id: herdrPluginId,
    ...(anchorPaneId ? { target_pane_id: anchorPaneId } : { workspace_id: team.workspace_id }),
  });
  if (!pane?.pane_id) {
    throw new SlpRuntimeError("runtime pane was not opened", ["plugin", "pane", "open"]);
  }
  return pane.pane_id;
}

export function recordRuntimePane(projectPath: string, teamId: string, generation: number, paneId: string): void {
  const store = openStore(projectPath);
  try {
    store.database
      .query(`UPDATE slp_local_teams SET runtime_pane_id = ? WHERE team_id = ? AND generation = ?`)
      .run(paneId, teamId, generation);
  } finally {
    store.close();
  }
}

function recordedRuntimePane(projectPath: string, teamId: string, generation: number): string {
  const store = openStore(projectPath, true);
  try {
    if (!store.hasColumn("slp_local_teams", "runtime_pane_id")) return "";
    return store.database
      .query<{ runtime_pane_id: string }, [string, number]>(
        `SELECT runtime_pane_id FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
      )
      .get(teamId, generation)?.runtime_pane_id ?? "";
  } finally {
    store.close();
  }
}

// Item 6 (d96, d759): after a Herdr restart, every RUNNING generation whose
// role panes survived gets its runtime pane back; one whose panes are all
// gone is noted as lost on the team card and nothing is re-created.
export async function runSlpRestore(
  environment: Record<string, string | undefined> = process.env,
  home = resolveHomeDirectory({ environmentHome: environment.HOME }),
): Promise<CliResult> {
  const client = new HerdrClient(environment);
  const lines: string[] = [];
  const restored: Array<Record<string, unknown>> = [];
  let agents: HerdrAgent[] | null = null;
  for (const projectPath of await registeredProjects(home)) {
    let generations: RunningGeneration[];
    try {
      generations = runningGenerations(projectPath);
    } catch {
      continue;
    }
    for (const generation of generations) {
      agents ??= await client.agentList();
      const label = `${generation.team.team_id}:g${generation.team.generation}`;
      const alive = generation.roles.filter((role) =>
        agents?.some((agent) => agent.name === role.name && agent.pane_id === role.pane_id)
      );
      if (alive.length === 0) {
        const store = openStore(projectPath);
        try {
          const card = teamCard(store, generation.team.team_id, generation.team.generation);
          const latest = card ? latestEntry(store, card.id) : null;
          if (card && latest?.flag !== "pane:lost") {
            recordEntry(
              store,
              generation.team,
              card.id,
              `lost: no role pane of ${label} survived the Herdr restart (${generation.roles.map((role) => `${role.name} ${role.pane_id}`).join(", ")}); store: ${generation.roles.length} roles recorded`,
              "pane:lost",
              "pane.lost",
            );
          }
        } finally {
          store.close();
        }
        lines.push(`${label}: lost, every role pane is gone; noted on the team card`);
        restored.push({ generation: label, outcome: "lost" });
        continue;
      }
      const directory = slpRuntimeDirectory(projectPath, generation.team.team_id, generation.team.generation);
      if (await runtimeLockHolder(directory) !== null) {
        lines.push(`${label}: runtime already running`);
        restored.push({ generation: label, outcome: "running" });
        continue;
      }
      const supervisor = alive.find((role) => role.role === "team-supervisor") ?? alive[0] ?? null;
      const paneId = await openRuntimePane(client, generation.team, supervisor?.pane_id ?? null);
      recordRuntimePane(projectPath, generation.team.team_id, generation.team.generation, paneId);
      lines.push(`${label}: runtime pane reopened as ${paneId} (${alive.length}/${generation.roles.length} role panes alive)`);
      restored.push({ generation: label, outcome: "reopened", paneId });
    }
  }
  return {
    data: { generations: restored },
    text: lines.length > 0 ? lines.join("\n") : "no running SLP generation to restore",
  };
}

// Item 2: the [[events]] safety net; a role pane dying while no runtime is
// subscribed still records the loss and wakes the Team Supervisor.
export async function runSlpEvent(
  environment: Record<string, string | undefined> = process.env,
  home = resolveHomeDirectory({ environmentHome: environment.HOME }),
): Promise<CliResult> {
  const raw = environment.HERDR_PLUGIN_EVENT_JSON;
  if (!raw) throw new CliError("INVALID_OPTION", "HERDR_PLUGIN_EVENT_JSON is required; maestro slp event runs as a Herdr plugin event hook");
  let envelope: { data?: Record<string, unknown>; event?: string };
  try {
    envelope = JSON.parse(raw) as typeof envelope;
  } catch {
    throw new CliError("INVALID_OPTION", "HERDR_PLUGIN_EVENT_JSON is not JSON");
  }
  const kind = String(envelope.event ?? environment.HERDR_PLUGIN_EVENT ?? "").replace(".", "_");
  const paneId = typeof envelope.data?.pane_id === "string" ? envelope.data.pane_id : null;
  if ((kind !== "pane_exited" && kind !== "pane_closed") || !paneId) {
    return { data: { handled: false }, text: `ignored ${kind || "event"}` };
  }
  for (const projectPath of await registeredProjects(home)) {
    let generations: RunningGeneration[];
    try {
      generations = runningGenerations(projectPath);
    } catch {
      continue;
    }
    for (const generation of generations) {
      const seat = generation.roles.find((role) => role.pane_id === paneId);
      if (!seat) continue;
      const label = `${generation.team.team_id}:g${generation.team.generation}`;
      const directory = slpRuntimeDirectory(projectPath, generation.team.team_id, generation.team.generation);
      if (await runtimeLockHolder(directory) !== null) {
        return { data: { handled: false, runtime: true }, text: `${label}: the runtime handles ${kind} for ${seat.name}` };
      }
      const runtime = new AttentionRuntime({ environment, generation: generation.team.generation, projectPath, teamId: generation.team.team_id });
      await runtime.load();
      const store = openStore(projectPath, true);
      try {
        runtime.refreshRoles(store);
      } finally {
        store.close();
      }
      await runtime.handle({ data: { pane_id: paneId, workspace_id: generation.team.workspace_id }, event: kind });
      return { data: { handled: true, seat: seat.name }, text: `${label}: recorded ${kind} for ${seat.name}` };
    }
  }
  return { data: { handled: false }, text: `no running generation owns pane ${paneId}` };
}

export async function slpRuntimeStatus(projectPath: string, teamId: string, generation: number): Promise<CliResult> {
  const directory = slpRuntimeDirectory(projectPath, teamId, generation);
  const holder = await runtimeLockHolder(directory);
  const state = await readRuntimeState(directory);
  const pending = Object.entries(state?.pending ?? {}).flatMap(([target, lines]) =>
    lines.map((line) => ({ line: line.line, queuedAt: line.queuedAt, subject: line.subject, target }))
  );
  return {
    data: {
      generation,
      pending,
      runtime: holder ? { pid: holder, state: "running" } : { pid: null, state: "not-running" },
      runtimePaneId: recordedRuntimePane(projectPath, teamId, generation) || null,
      teamId,
    },
    text: [
      `${teamId} g${generation} runtime ${holder ? `running (pid ${holder})` : "not running"}`,
      ...(pending.length === 0 ? ["pending: none"] : pending.map((line) => `pending ${line.target}: ${line.line}`)),
    ].join("\n"),
  };
}
