import { createHash, randomUUID } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOption,
  type CliGuard,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { PluginContext } from "../kernel/loader.ts";
import { Store, resolveStoreLocation, tableExists } from "../kernel/store.ts";
import { resolveHomeDirectory } from "./home.ts";
import { isRoom } from "./room.ts";
import type { DecisionSection } from "./decision.ts";
import type { InstallEvent } from "./install.ts";
import { updateEntry } from "./plugin-manager.ts";
import type { RecipeService } from "./recipe.ts";
import { installMachineFiles, removeMachineFiles, seatTokenCheck, syncSeatFiles } from "./slp-plugin.ts";
import { resolveSeat, seatDigest, seatFileDirectories, seatNames, writeSeatToken, type Seat } from "./slp-seats.ts";
import {
  buildSlpTeamPlan,
  HerdrSlpRuntime,
  type SeatLaunch,
  type SlpAcknowledgedRole,
  type SlpRole,
  type SlpRoleContract,
  type SlpRolePlan,
  type SlpRuntimeRole,
  type SlpRuntimeStart,
  type SlpTeamPlan,
} from "./slp-runtime.ts";
import { registerSessionCommand } from "./session-required.ts";
import { compactLine, compactsForPane, currentPaneId } from "./session-telemetry.ts";
import {
  openRuntimePane,
  recordRuntimePane,
  runSlpRestore,
  runSlpRuntime,
  runtimeConfigFromEnvironment,
  slpRuntimeStatus,
} from "./slp-attention.ts";

type WorkState = "OPEN" | "ACTIVE" | "RETURNED" | "DONE";
type LifecycleOperation = "START" | "STOP";
type LifecyclePhase = "RESERVED" | "RUNTIME_READY" | "COMMITTED";

interface PackModels {
  lead?: string;
  peer?: string;
}

// profileDigests keeps its stored name so generations started before d906 still read.
interface PackConfiguration {
  models?: PackModels;
  profileDigests: Record<string, string>;
}

function packModels(value: unknown): PackModels | undefined {
  if (!value || typeof value !== "object") return undefined;
  const raw = value as Record<string, unknown>;
  const models: PackModels = {};
  for (const seat of seatNames) {
    const model = raw[seat];
    if (typeof model === "string" && model.trim() !== "") models[seat] = model;
  }
  return Object.keys(models).length > 0 ? models : undefined;
}

function packConfiguration(json: string): PackConfiguration {
  const parsed = JSON.parse(json) as Partial<PackConfiguration>;
  const models = packModels(parsed.models);
  return {
    ...(models ? { models } : {}),
    profileDigests: parsed.profileDigests ?? {},
  };
}

function seatLaunch(seat: Seat, model: string | undefined): SeatLaunch {
  return { autocompact: seat.config.autocompact ?? null, harness: seat.config.harness, model: model ?? null };
}

async function resolveSeats(projectPath: string, models: PackModels = {}): Promise<{ lead: SeatLaunch; profileDigests: Record<string, string> }> {
  const home = resolveHomeDirectory();
  const lead = await resolveSeat("lead", projectPath, home);
  const peer = await resolveSeat("peer", projectPath, home);
  return { lead: seatLaunch(lead, models.lead), profileDigests: { lead: seatDigest(lead), peer: seatDigest(peer) } };
}

// A seat file that no longer parses must not stop a team from being inspected or closed.
async function planForTeam(team: {
  configuration_json: string;
  generation: number;
  project_path: string;
  team_id: string;
}): Promise<SlpTeamPlan> {
  const { models = {} } = packConfiguration(team.configuration_json);
  const lead = await resolveSeat("lead", team.project_path, resolveHomeDirectory())
    .then((seat) => seatLaunch(seat, models.lead))
    .catch((): SeatLaunch => ({ autocompact: null, harness: "codex", model: models.lead ?? null }));
  return buildSlpTeamPlan({
    generation: team.generation,
    lead,
    projectPath: team.project_path,
    teamId: team.team_id,
  });
}

// A pin under a name that is no longer a seat (a peer variant from before d906) has nothing left to compare.
async function requireSeatsUnchanged(
  configuration: PackConfiguration,
  team: { generation: number; project_path: string; team_id: string },
): Promise<void> {
  for (const name of seatNames) {
    const digest = configuration.profileDigests[name];
    if (digest === undefined) continue;
    const seat = await resolveSeat(name, team.project_path, resolveHomeDirectory());
    const actual = seatDigest(seat);
    if (actual === digest) continue;
    const where = seat.paths.length > 1 ? `${seat.paths[0]} (frontmatter) and ${seat.paths[1]} (mandate)` : seat.paths[0];
    throw new CliError(
      "SLP_SNAPSHOT_CHANGED",
      `running generation ${team.team_id}:g${team.generation} must keep its pinned ${name} seat file, whose bytes are decided by ${where}; put those bytes back as they were pinned, or stop the generation with maestro team stop ${team.team_id}`,
      { actual, deciding: seat.paths, expected: digest, searched: seatFileDirectories(team.project_path, resolveHomeDirectory()), seat: name },
    );
  }
}

interface SlpWorkRecord {
  assignedTo: string;
  createdAt: string;
  createdBy: string;
  generation: number;
  id: string;
  objective: string;
  owner: string | null;
  state: WorkState;
  teamId: string;
  updatedAt: string;
}

interface SlpLifecycleRow {
  actor: string;
  configuration_json: string;
  created_at: string;
  emergency: number;
  generation: number;
  objective: string;
  operation: LifecycleOperation;
  owner_pid: number | null;
  owner_token: string | null;
  pack_digest: string;
  pack_version: string;
  phase: LifecyclePhase;
  project_path: string;
  reason: string;
  revision: number;
  runtime_json: string | null;
  team_id: string;
  updated_at: string;
  work_id: string;
  workspace_id: string | null;
}

const nonTeamVerbs = new Map<string, string>([
  ["attention", "status"],
  ["brief", "status"],
  ["decision", "decide"],
  ["decision list", "status"],
  ["decision show", "status <decision-id>"],
  ["work done", "work return then work accept"],
  ["work list", "status"],
  ["work reclaim", "work take"],
  ["work release", "work return"],
  ["work show", "status <work-id>"],
  ["work start", "work take"],
]);

function canonicalCheckoutRoot(cwd: string): string {
  return realpathSync.native(resolveStoreLocation(cwd).root);
}

function paneGuard(store: Store, cwd = process.cwd()): CliGuard {
  return {
    beforeInvoke(command) {
      const replacement = [...nonTeamVerbs.entries()]
        .filter(([verb]) => command === verb || command.startsWith(`${verb} `))
        .sort(([left], [right]) => right.length - left.length)[0]?.[1];
      if (!replacement) return;
      if (!tableExists(store, "slp_local_teams") || !tableExists(store, "slp_local_roles")) return;
      const paneId = currentPaneId();
      if (!paneId) return;
      const activeRole = store.database
        .query<{ present: number }, [string, string]>(
          `SELECT 1 AS present
           FROM slp_local_teams AS team
           JOIN slp_local_roles AS role
             ON role.team_id = team.team_id AND role.generation = team.generation
           WHERE team.project_path = ? AND team.state = 'RUNNING' AND role.pane_id = ?
           LIMIT 1`,
        )
        .get(canonicalCheckoutRoot(cwd), paneId);
      if (activeRole) {
        throw new CliError(
          "SLP_V2_CUTOVER",
          `${command} was retired by SLP v2; use maestro ${replacement}`,
          { command, replacement },
        );
      }
    },
  };
}

function migrateRoom(store: Store): void {
  store.migrate(`
    CREATE TABLE IF NOT EXISTS slp_teams (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      state TEXT NOT NULL CHECK(state IN ('RUNNING', 'STOPPED')),
      workspace_id TEXT NOT NULL,
      created_at TEXT NOT NULL,
      stopped_at TEXT,
      PRIMARY KEY(team_id, generation),
      UNIQUE(project_path, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_team_roles (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('lead', 'peer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      instance_id TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      brief_digest TEXT NOT NULL,
      ready_challenge TEXT NOT NULL,
      profile TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name),
      FOREIGN KEY(team_id, generation) REFERENCES slp_teams(team_id, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_activity (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      actor TEXT NOT NULL,
      operation TEXT NOT NULL,
      target_type TEXT NOT NULL,
      target_id TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_lifecycle_operations (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      operation TEXT NOT NULL CHECK(operation IN ('START', 'STOP')),
      phase TEXT NOT NULL CHECK(phase IN ('RESERVED', 'RUNTIME_READY', 'COMMITTED')),
      revision INTEGER NOT NULL CHECK(revision > 0),
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      work_id TEXT NOT NULL,
      workspace_id TEXT,
      runtime_json TEXT,
      actor TEXT NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      emergency INTEGER NOT NULL CHECK(emergency IN (0, 1)),
      owner_token TEXT,
      owner_pid INTEGER,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, operation),
      UNIQUE(project_path, generation, operation)
    );
    CREATE TABLE IF NOT EXISTS slp_decisions (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      choice TEXT NOT NULL,
      why TEXT NOT NULL,
      scope TEXT NOT NULL,
      work_id TEXT,
      replaces_id TEXT,
      actor TEXT NOT NULL,
      created_at TEXT NOT NULL,
      provisional INTEGER NOT NULL DEFAULT 0
    );
  `);
}

interface ProvisionalDecision {
  choice: string;
  createdAt: string;
  generation: number;
  id: string;
  teamId: string;
  workId: string | null;
}

// The column may be missing on a room store no Hub command has migrated since
// the update; a reader never fails on it.
function provisionalColumn(store: Store): string {
  return store.hasColumn("slp_decisions", "provisional") ? "provisional" : "0 AS provisional";
}

function provisionalDecisions(store: Store): ProvisionalDecision[] {
  if (!tableExists(store, "slp_decisions") || !store.hasColumn("slp_decisions", "provisional")) {
    return [];
  }
  return store.database
    .query<{
      choice: string;
      created_at: string;
      generation: number;
      id: string;
      team_id: string;
      work_id: string | null;
    }, []>(
      `SELECT id, team_id, generation, work_id, choice, created_at
       FROM slp_decisions WHERE provisional = 1 ORDER BY created_at, id`,
    )
    .all()
    .map((row) => ({
      choice: row.choice,
      createdAt: row.created_at,
      generation: row.generation,
      id: row.id,
      teamId: row.team_id,
      workId: row.work_id,
    }));
}

// A Hub pane not named `supervisor` never receives the BLOCKED push, so a standing escalation has to show in the store.
function blockedForHub(
  room: Store,
  project: Store,
  team: { generation: number; team_id: string },
  roles: ReadonlyArray<{ name: string; role: string }>,
): Array<{ body: string; workId: string }> {
  const escalator = roles.find((role) => role.role === "lead");
  if (!escalator || !tableExists(project, "slp_work_entries")) return [];
  if (!project.hasColumn("slp_work_entries", "flag")) return [];
  const notes = project.database
    .query<{ body: string; created_at: string; work_id: string }, [string, number, string, string]>(
      `SELECT e.work_id, e.body, e.created_at FROM slp_work_entries e
       JOIN slp_work w ON w.id = e.work_id
       WHERE w.team_id = ? AND w.generation = ? AND e.actor = ? AND e.flag = 'blocked'
         AND e.id = (SELECT MAX(x.id) FROM slp_work_entries x WHERE x.work_id = e.work_id AND x.actor = ?)
       ORDER BY e.id`,
    )
    .all(team.team_id, team.generation, escalator.name, escalator.name);
  const ruled = room.database.query<{ present: number }, [string, number, string, string]>(
    `SELECT 1 AS present FROM slp_decisions
     WHERE team_id = ? AND generation = ? AND work_id = ? AND created_at > ? LIMIT 1`,
  );
  return notes
    .filter((note) => !ruled.get(team.team_id, team.generation, note.work_id, note.created_at))
    .map((note) => ({ body: note.body, workId: note.work_id }));
}

function provisionalLines(decisions: ProvisionalDecision[]): string[] {
  return decisions.map((decision) =>
    `provisional: ${decision.id} ${decision.workId ?? "none"} ${clipLine(decision.choice, 120)}`
  );
}

function migrateProject(store: Store): void {
  store.migrate(`
    CREATE TABLE IF NOT EXISTS slp_local_teams (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      room_store_path TEXT NOT NULL,
      project_path TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      state TEXT NOT NULL CHECK(state IN ('RUNNING', 'STOPPED')),
      workspace_id TEXT NOT NULL,
      bound_at TEXT NOT NULL,
      runtime_pane_id TEXT NOT NULL DEFAULT '',
      supervisor_pane_id TEXT NOT NULL DEFAULT '',
      PRIMARY KEY(team_id, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_local_roles (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('lead', 'peer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      instance_id TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      brief_digest TEXT NOT NULL,
      ready_challenge TEXT NOT NULL,
      profile TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name)
    );
    CREATE TABLE IF NOT EXISTS slp_work (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      objective TEXT NOT NULL,
      created_by TEXT NOT NULL,
      assigned_to TEXT NOT NULL,
      owner TEXT,
      state TEXT NOT NULL CHECK(state IN ('OPEN', 'ACTIVE', 'RETURNED', 'DONE')),
      current_return TEXT,
      return_revision INTEGER NOT NULL DEFAULT 0,
      abandoned_at TEXT,
      abandoned_by TEXT,
      abandonment_reason TEXT,
      acceptance_outcome TEXT,
      accepted_by TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_work_entries (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      kind TEXT NOT NULL CHECK(kind IN ('NOTE', 'RETURN', 'ACCEPTANCE')),
      actor TEXT NOT NULL,
      body TEXT NOT NULL,
      created_at TEXT NOT NULL,
      flag TEXT
    );
    CREATE TABLE IF NOT EXISTS slp_rework_grants (
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      return_revision INTEGER NOT NULL CHECK(return_revision > 0),
      reviewer TEXT NOT NULL,
      granted_at TEXT NOT NULL,
      consumed_at TEXT,
      PRIMARY KEY(work_id, return_revision)
    );
    CREATE TABLE IF NOT EXISTS slp_lesson_candidates (
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      source TEXT NOT NULL,
      recorded_at TEXT NOT NULL,
      PRIMARY KEY(work_id, source)
    );
    CREATE TABLE IF NOT EXISTS slp_decisions (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      choice TEXT NOT NULL,
      why TEXT NOT NULL,
      scope TEXT NOT NULL,
      work_id TEXT,
      replaces_id TEXT,
      actor TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_activity (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      actor TEXT NOT NULL,
      operation TEXT NOT NULL,
      target_type TEXT NOT NULL,
      target_id TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_lifecycle_operations (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      operation TEXT NOT NULL CHECK(operation IN ('START', 'STOP')),
      phase TEXT NOT NULL CHECK(phase IN ('RESERVED', 'RUNTIME_READY', 'COMMITTED')),
      revision INTEGER NOT NULL CHECK(revision > 0),
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      work_id TEXT NOT NULL,
      workspace_id TEXT,
      runtime_json TEXT,
      actor TEXT NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      emergency INTEGER NOT NULL CHECK(emergency IN (0, 1)),
      owner_token TEXT,
      owner_pid INTEGER,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, operation),
      UNIQUE(project_path, generation, operation)
    );
  `);
}

function slug(value: string): string {
  const normalized = value.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "");
  if (!normalized) throw new CliError("INVALID_TEAM_ID", "project basename cannot form a team id");
  return normalized;
}

function teamIdForProject(projectPath: string): string {
  const readable = slug(basename(projectPath)).slice(0, 10).replace(/-$/, "");
  const canonicalPath = realpathSync.native(projectPath);
  const identity = createHash("sha256").update(canonicalPath).digest("hex").slice(0, 10);
  return `${readable}-${identity}`;
}

function packVersion(pack: string): string {
  const version = /<!-- slp:version=([^\s]+) -->/.exec(pack)?.[1];
  if (!version) throw new CliError("INVALID_SLP_PACK", "SLP.md is missing its version marker");
  return version;
}

// d90: the mandate lives in the seat file, so the post-open prompt carries only
// team, generation, instance and the ready challenge.
function roleContracts(
  teamId: string,
  generation: number,
  packDigest: string,
  existingInstances: Partial<Record<SlpRole, string>> = {},
): Map<SlpRole, SlpRoleContract> {
  const contract = (role: SlpRole): SlpRoleContract => {
    const instanceId = existingInstances[role] || randomUUID();
    const readyChallenge = randomUUID().replaceAll("-", "");
    const body = `slp team ${teamId} generation ${generation} instance ${instanceId}; reply ${readyChallenge}`;
    const briefDigest = createHash("sha256").update(body).digest("hex");
    const acknowledgement = [
      "SLP_ROLE_READY",
      `team=${teamId}`,
      `generation=${generation}`,
      `role=${role}`,
      `challenge=${readyChallenge}`,
    ].join(" ");
    return { acknowledgement, body, briefDigest, instanceId, packDigest, readyChallenge };
  };
  return new Map<SlpRole, SlpRoleContract>([
    ["lead", contract("lead")],
    ["peer", contract("peer")],
  ]);
}

async function archivePack(roomRoot: string, bytes: Uint8Array, digest: string): Promise<string> {
  const directory = join(roomRoot, ".maestro", "packs");
  const path = join(directory, `${digest}.md`);
  await mkdir(directory, { recursive: true });
  if (!existsSync(path)) {
    const temporary = join(directory, `.${digest}.${randomUUID()}.tmp`);
    try {
      await writeFile(temporary, bytes, { flag: "wx" });
      await rename(temporary, path);
    } finally {
      await rm(temporary, { force: true });
    }
  }
  const archived = new Uint8Array(await readFile(path));
  const archivedDigest = createHash("sha256").update(archived).digest("hex");
  if (archivedDigest !== digest) {
    throw new CliError(
      "SLP_PACK_ARCHIVE_CORRUPT",
      `archived Workspace Pack ${path} does not match digest ${digest}`,
      { actual: archivedDigest, expected: digest, path },
    );
  }
  return path;
}

function toWork(row: {
  assigned_to: string;
  created_at: string;
  created_by: string;
  generation: number;
  id: string;
  objective: string;
  owner: string | null;
  state: WorkState;
  team_id: string;
  updated_at: string;
}): SlpWorkRecord {
  return {
    assignedTo: row.assigned_to,
    createdAt: row.created_at,
    createdBy: row.created_by,
    generation: row.generation,
    id: row.id,
    objective: row.objective,
    owner: row.owner,
    state: row.state,
    teamId: row.team_id,
    updatedAt: row.updated_at,
  };
}

async function restoreSnapshot(path: string, previous: Uint8Array | null): Promise<void> {
  if (previous === null) {
    await rm(path, { force: true });
  } else {
    await writeFile(path, previous);
  }
}

function withImmediateTransaction<T>(store: Store, action: () => T): T {
  store.database.exec("BEGIN IMMEDIATE");
  try {
    const result = action();
    store.database.exec("COMMIT");
    return result;
  } catch (error) {
    try {
      store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
}

function lifecycleOwnerIsAlive(ownerPid: number | null): boolean {
  if (!ownerPid) return false;
  try {
    process.kill(ownerPid, 0);
    return true;
  } catch {
    return false;
  }
}

function lifecycleRow(
  store: Store,
  teamId: string,
  generation: number,
  operation: LifecycleOperation,
): SlpLifecycleRow | null {
  return store.database
    .query<SlpLifecycleRow, [string, number, LifecycleOperation]>(
      `SELECT * FROM slp_lifecycle_operations
       WHERE team_id = ? AND generation = ? AND operation = ?`,
    )
    .get(teamId, generation, operation) ?? null;
}

function runtimePaneIdOf(store: Store, teamId: string, generation: number): string {
  return store.database
    .query<{ runtime_pane_id: string }, [string, number]>(
      "SELECT runtime_pane_id FROM slp_local_teams WHERE team_id = ? AND generation = ?",
    )
    .get(teamId, generation)?.runtime_pane_id ?? "";
}

function lifecycleRuntimeRoles(row: SlpLifecycleRow): SlpRuntimeRole[] | null {
  if (!row.runtime_json) return null;
  const parsed = JSON.parse(row.runtime_json) as unknown;
  if (!Array.isArray(parsed)) {
    throw new CliError(
      "SLP_LIFECYCLE_CORRUPT",
      `${row.team_id}:g${row.generation} has invalid runtime recovery data`,
    );
  }
  return parsed as SlpRuntimeRole[];
}

function updateLifecycleOwner(
  store: Store,
  row: Pick<SlpLifecycleRow, "generation" | "operation" | "team_id">,
  ownerToken: string,
  ownerPid: number,
  replacement: Pick<
    SlpLifecycleRow,
    "configuration_json" | "objective" | "pack_digest" | "pack_version"
  > | null = null,
): SlpLifecycleRow {
  const now = new Date().toISOString();
  const update = (table: string) =>
    replacement
      ? store.database
        .query(
          `UPDATE ${table}
           SET owner_token = ?, owner_pid = ?, revision = revision + 1, updated_at = ?,
               objective = ?, configuration_json = ?, pack_digest = ?, pack_version = ?
           WHERE team_id = ? AND generation = ? AND operation = ?
             AND phase = 'RESERVED'`,
        )
        .run(
          ownerToken,
          ownerPid,
          now,
          replacement.objective,
          replacement.configuration_json,
          replacement.pack_digest,
          replacement.pack_version,
          row.team_id,
          row.generation,
          row.operation,
        )
      : store.database
        .query(
          `UPDATE ${table}
           SET owner_token = ?, owner_pid = ?, revision = revision + 1, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = ?
             AND phase <> 'COMMITTED'`,
        )
        .run(ownerToken, ownerPid, now, row.team_id, row.generation, row.operation);
  const local = update("slp_lifecycle_operations");
  const room = update("slp_room.slp_lifecycle_operations");
  if (local.changes !== 1 || room.changes !== 1) {
    throw new CliError(
      "SLP_LIFECYCLE_CHANGED",
      `${row.team_id}:g${row.generation} ${row.operation} changed before it could be claimed`,
    );
  }
  const claimed = lifecycleRow(store, row.team_id, row.generation, row.operation);
  if (!claimed) throw new Error("claimed SLP lifecycle operation disappeared");
  return claimed;
}

function releaseLifecycleOwner(
  store: Store,
  row: Pick<SlpLifecycleRow, "generation" | "operation" | "team_id">,
  ownerToken: string,
): void {
  withImmediateTransaction(store, () => {
    const now = new Date().toISOString();
    const release = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET owner_token = NULL, owner_pid = NULL,
               revision = revision + 1, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = ?
             AND owner_token = ? AND phase <> 'COMMITTED'`,
        )
        .run(now, row.team_id, row.generation, row.operation, ownerToken);
    release("slp_lifecycle_operations");
    release("slp_room.slp_lifecycle_operations");
  });
}

function claimRunningStartRepair(
  store: Store,
  row: Pick<SlpLifecycleRow, "generation" | "team_id">,
  ownerToken: string,
): SlpLifecycleRow {
  const now = new Date().toISOString();
  const claim = (table: string) =>
    store.database
      .query(
        `UPDATE ${table}
         SET owner_token = ?, owner_pid = ?, revision = revision + 1, updated_at = ?
         WHERE team_id = ? AND generation = ? AND operation = 'START'
           AND phase = 'COMMITTED'`,
      )
      .run(ownerToken, process.pid, now, row.team_id, row.generation);
  const local = claim("slp_lifecycle_operations");
  const room = claim("slp_room.slp_lifecycle_operations");
  if (local.changes !== 1 || room.changes !== 1) {
    throw new CliError(
      "SLP_LIFECYCLE_CHANGED",
      `${row.team_id}:g${row.generation} changed before runtime repair could be claimed`,
    );
  }
  const claimed = lifecycleRow(store, row.team_id, row.generation, "START");
  if (!claimed) throw new Error("claimed SLP runtime repair disappeared");
  return claimed;
}

function releaseRunningStartRepair(
  store: Store,
  row: Pick<SlpLifecycleRow, "generation" | "team_id">,
  ownerToken: string,
): void {
  withImmediateTransaction(store, () => {
    const now = new Date().toISOString();
    const release = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET owner_token = NULL, owner_pid = NULL,
               revision = revision + 1, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = 'START'
             AND owner_token = ? AND phase = 'COMMITTED'`,
        )
        .run(now, row.team_id, row.generation, ownerToken);
    const local = release("slp_lifecycle_operations");
    const room = release("slp_room.slp_lifecycle_operations");
    if (local.changes !== 1 || room.changes !== 1) {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${row.team_id}:g${row.generation} changed before runtime repair could be released`,
      );
    }
  });
}

interface RunningTeamRow {
  configuration_json: string;
  generation: number;
  objective: string;
  pack_digest: string;
  pack_version: string;
  project_path: string;
  team_id: string;
  workspace_id: string;
}

type StartReservation =
  | { kind: "claimed"; row: SlpLifecycleRow }
  | { kind: "running"; operation: SlpLifecycleRow; row: RunningTeamRow }
  | { kind: "wait" };

// room d128: a start that names a different model set than the pinned one is a
// configuration change, refused on a running or pending generation exactly as
// an objective change is.
function changedConfiguration(
  input: { models: PackModels },
  configuration: PackConfiguration,
): boolean {
  const pinned = configuration.models ?? {};
  return (["lead", "peer"] as const).some((seat) => (input.models[seat] ?? null) !== (pinned[seat] ?? null));
}

function reserveStart(
  store: Store,
  input: {
    configuration: PackConfiguration;
    digest: string;
    models: PackModels;
    objective: string;
    projectPath: string;
    teamId: string;
    version: string;
  },
  ownerToken: string,
): StartReservation {
  return withImmediateTransaction(store, () => {
    const collision = store.database
      .query<{ project_path: string }, [string, string]>(
        `SELECT project_path FROM slp_room.slp_teams
         WHERE team_id = ? AND project_path <> ? LIMIT 1`,
      )
      .get(input.teamId, input.projectPath);
    if (collision) {
      throw new CliError(
        "TEAM_ID_COLLISION",
        `${input.teamId} already identifies another project`,
        { existingProjectPath: collision.project_path, projectPath: input.projectPath },
      );
    }

    const running = store.database
      .query<RunningTeamRow, [string]>(
        `SELECT team_id, generation, objective, project_path, configuration_json,
                pack_version, pack_digest, workspace_id
         FROM slp_room.slp_teams
         WHERE project_path = ? AND state = 'RUNNING'
         ORDER BY generation DESC LIMIT 1`,
      )
      .get(input.projectPath);
    if (running) {
      const configuration = packConfiguration(running.configuration_json);
      if (running.objective !== input.objective || changedConfiguration(input, configuration)) {
        throw new CliError(
          "TEAM_RUNNING",
          `${running.team_id} is already running; run maestro team stop before changing its objective or configuration`,
          {
            generation: running.generation,
            objective: running.objective,
            projectPath: running.project_path,
          },
        );
      }
      const stopping = lifecycleRow(store, running.team_id, running.generation, "STOP");
      if (
        stopping &&
        stopping.phase !== "COMMITTED" &&
        stopping.owner_token &&
        lifecycleOwnerIsAlive(stopping.owner_pid)
      ) {
        throw new CliError(
          "TEAM_STOP_IN_PROGRESS",
          `${running.team_id}:g${running.generation} is shutting down; runtime repair is fenced`,
        );
      }
      const operation = lifecycleRow(store, running.team_id, running.generation, "START");
      if (!operation || operation.phase !== "COMMITTED") {
        throw new CliError(
          "SLP_LIFECYCLE_CORRUPT",
          `${running.team_id}:g${running.generation} has no committed START lifecycle record`,
        );
      }
      if (
        operation.owner_token &&
        lifecycleOwnerIsAlive(operation.owner_pid)
      ) {
        return { kind: "wait" };
      }
      return {
        kind: "running",
        operation: claimRunningStartRepair(store, operation, ownerToken),
        row: running,
      };
    }

    const pending = store.database
      .query<SlpLifecycleRow, [string]>(
        `SELECT * FROM slp_room.slp_lifecycle_operations
         WHERE project_path = ? AND operation = 'START' AND phase <> 'COMMITTED'
         ORDER BY generation DESC LIMIT 1`,
      )
      .get(input.projectPath);
    if (pending) {
      const changed = pending.objective !== input.objective ||
        changedConfiguration(input, packConfiguration(pending.configuration_json));
      const owned = Boolean(pending.owner_token) && lifecycleOwnerIsAlive(pending.owner_pid);
      // A reservation whose owner died pins nothing before RUNTIME_READY: the
      // retry may bring another objective or model set.
      if (changed && (owned || pending.phase !== "RESERVED")) {
        throw new CliError(
          "TEAM_START_PENDING",
          `${pending.team_id}:g${pending.generation} is already starting with another objective or configuration`,
          { generation: pending.generation, objective: pending.objective },
        );
      }
      if (owned) return { kind: "wait" };
      return {
        kind: "claimed",
        row: updateLifecycleOwner(
          store,
          pending,
          ownerToken,
          process.pid,
          changed
            ? {
              configuration_json: JSON.stringify(input.configuration),
              objective: input.objective,
              pack_digest: input.digest,
              pack_version: input.version,
            }
            : null,
        ),
      };
    }

    const generation = (store.database
      .query<{ generation: number }, [string, string, string, string]>(
        `SELECT MAX(generation) AS generation FROM (
           SELECT generation FROM slp_room.slp_teams WHERE team_id = ? OR project_path = ?
           UNION ALL
           SELECT generation FROM slp_room.slp_lifecycle_operations
             WHERE team_id = ? OR project_path = ?
         )`,
      )
      .get(input.teamId, input.projectPath, input.teamId, input.projectPath)?.generation ?? 0) + 1;
    const workId = store.nextCountedId("w", ["work", "slp_work", "slp_lifecycle_operations.work_id"]);
    const now = new Date().toISOString();
    const values = [
      input.teamId,
      generation,
      "START",
      "RESERVED",
      1,
      input.projectPath,
      input.objective,
      JSON.stringify(input.configuration),
      input.version,
      input.digest,
      workId,
      "hub-supervisor",
      0,
      ownerToken,
      process.pid,
      now,
      now,
    ] as const;
    const insert = (table: string) =>
      store.database
        .query(
          `INSERT INTO ${table}
            (team_id, generation, operation, phase, revision, project_path,
             objective, configuration_json, pack_version, pack_digest, work_id,
             actor, emergency, owner_token, owner_pid, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .run(...values);
    insert("slp_lifecycle_operations");
    insert("slp_room.slp_lifecycle_operations");
    const row = lifecycleRow(store, input.teamId, generation, "START");
    if (!row) throw new Error("reserved SLP start disappeared");
    return { kind: "claimed", row };
  });
}

function recordStartRuntimeReady(
  store: Store,
  row: SlpLifecycleRow,
  ownerToken: string,
  started: SlpRuntimeStart,
): SlpLifecycleRow {
  return withImmediateTransaction(store, () => {
    const now = new Date().toISOString();
    const runtimeJson = JSON.stringify(started.roles);
    const update = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET phase = 'RUNTIME_READY', revision = revision + 1,
               workspace_id = ?, runtime_json = ?, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = 'START'
             AND owner_token = ? AND phase IN ('RESERVED', 'RUNTIME_READY')`,
        )
        .run(
          started.workspaceId,
          runtimeJson,
          now,
          row.team_id,
          row.generation,
          ownerToken,
        );
    const local = update("slp_lifecycle_operations");
    const room = update("slp_room.slp_lifecycle_operations");
    if (local.changes !== 1 || room.changes !== 1) {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${row.team_id}:g${row.generation} changed before runtime readiness could be recorded`,
      );
    }
    const ready = lifecycleRow(store, row.team_id, row.generation, "START");
    if (!ready) throw new Error("runtime-ready SLP start disappeared");
    return ready;
  });
}

function refreshStartRuntime(
  store: Store,
  row: Pick<SlpLifecycleRow, "generation" | "team_id">,
  ownerToken: string,
  started: SlpRuntimeStart,
  now: string,
): void {
  const runtimeJson = JSON.stringify(started.roles);
  const update = (table: string) =>
    store.database
      .query(
        `UPDATE ${table}
         SET workspace_id = ?, runtime_json = ?, revision = revision + 1, updated_at = ?
         WHERE team_id = ? AND generation = ? AND operation = 'START'
           AND owner_token = ? AND phase = 'COMMITTED'`,
      )
      .run(started.workspaceId, runtimeJson, now, row.team_id, row.generation, ownerToken);
  const local = update("slp_lifecycle_operations");
  const room = update("slp_room.slp_lifecycle_operations");
  if (local.changes !== 1 || room.changes !== 1) {
    throw new CliError(
      "SLP_LIFECYCLE_CHANGED",
      `${row.team_id}:g${row.generation} changed before its runtime snapshot could be refreshed`,
    );
  }
}

function upsertStartedRoles(
  store: Store,
  table: "slp_local_roles" | "slp_room.slp_team_roles",
  teamId: string,
  generation: number,
  roles: readonly SlpRuntimeRole[],
  now: string,
): void {
  const statement = store.database.query(
    `INSERT INTO ${table}
      (team_id, generation, role, name, pane_id, workspace_id, instance_id,
       pack_digest, brief_digest, ready_challenge, profile, created_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(team_id, generation, name) DO UPDATE SET
       role = excluded.role,
       pane_id = excluded.pane_id,
       workspace_id = excluded.workspace_id,
       instance_id = excluded.instance_id,
       pack_digest = excluded.pack_digest,
       brief_digest = excluded.brief_digest,
       ready_challenge = excluded.ready_challenge,
       profile = excluded.profile`,
  );
  for (const role of roles) {
    statement.run(
      teamId,
      generation,
      role.role,
      role.name,
      role.paneId,
      role.workspaceId,
      role.instanceId,
      role.packDigest,
      role.briefDigest,
      role.readyChallenge,
      role.profile,
      now,
    );
  }
}

function initialWorkRow(store: Store, teamId: string, generation: number) {
  return store.database
    .query<{
      assigned_to: string;
      created_at: string;
      created_by: string;
      generation: number;
      id: string;
      objective: string;
      owner: string | null;
      state: WorkState;
      team_id: string;
      updated_at: string;
    }, [string, number]>(
      `SELECT * FROM slp_work
       WHERE team_id = ? AND generation = ? AND created_by = 'hub-supervisor'
       ORDER BY created_at LIMIT 1`,
    )
    .get(teamId, generation) ?? null;
}

function finalizeStart(
  store: Store,
  row: SlpLifecycleRow,
  ownerToken: string,
  roles: readonly SlpRuntimeRole[],
  roomStorePath: string,
) {
  return withImmediateTransaction(store, () => {
    const current = lifecycleRow(store, row.team_id, row.generation, "START");
    if (!current || current.owner_token !== ownerToken || current.phase !== "RUNTIME_READY") {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${row.team_id}:g${row.generation} is not ready for start finalization`,
      );
    }
    const lead = roles.find((role) => role.role === "lead");
    if (!lead) throw new CliError("RUNTIME_INCOMPLETE", "Lead was not ready after team start");
    const now = new Date().toISOString();
    store.database
      .query(
        `INSERT INTO slp_local_teams
          (team_id, generation, room_store_path, project_path,
           configuration_json, pack_version, pack_digest, state, workspace_id,
           supervisor_pane_id, bound_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?, ?)`,
      )
      .run(
        current.team_id,
        current.generation,
        roomStorePath,
        current.project_path,
        current.configuration_json,
        current.pack_version,
        current.pack_digest,
        current.workspace_id,
        currentPaneId() ?? "",
        now,
      );
    upsertStartedRoles(store, "slp_local_roles", current.team_id, current.generation, roles, now);
    store.database
      .query(
        `INSERT INTO slp_work
          (id, team_id, generation, objective, created_by, assigned_to, owner,
           state, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'hub-supervisor', ?, NULL, 'OPEN', ?, ?)`,
      )
      .run(current.work_id, current.team_id, current.generation, current.objective, lead.name, now, now);
    store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, 'hub-supervisor', 'work.add', 'work', ?, ?)`,
      )
      .run(current.team_id, current.generation, current.work_id, now);
    store.database
      .query(
        `INSERT INTO slp_room.slp_teams
          (team_id, generation, project_path, objective, configuration_json,
           pack_version, pack_digest, state, workspace_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?)`,
      )
      .run(
        current.team_id,
        current.generation,
        current.project_path,
        current.objective,
        current.configuration_json,
        current.pack_version,
        current.pack_digest,
        current.workspace_id,
        now,
      );
    upsertStartedRoles(
      store,
      "slp_room.slp_team_roles",
      current.team_id,
      current.generation,
      roles,
      now,
    );
    store.database
      .query(
        `INSERT INTO slp_room.slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, 'hub-supervisor', 'team.start', 'team', ?, ?)`,
      )
      .run(current.team_id, current.generation, `${current.team_id}:g${current.generation}`, now);
    const commitLifecycle = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET phase = 'COMMITTED', revision = revision + 1,
               owner_token = NULL, owner_pid = NULL, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = 'START'
             AND owner_token = ? AND phase = 'RUNTIME_READY'`,
        )
        .run(now, current.team_id, current.generation, ownerToken);
    const local = commitLifecycle("slp_lifecycle_operations");
    const room = commitLifecycle("slp_room.slp_lifecycle_operations");
    if (local.changes !== 1 || room.changes !== 1) {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${current.team_id}:g${current.generation} changed before start could commit`,
      );
    }
    const work = initialWorkRow(store, current.team_id, current.generation);
    if (!work) throw new Error(`created work disappeared: ${current.work_id}`);
    return work;
  });
}

// Item 3 (d96): one runtime pane per generation, opened once the generation is
// RUNNING and reopened by repair (d759) when it is gone; a generation without
// its runtime is refused loudly rather than left looking watched (doctrine D7).
async function ensureRuntimePane(
  runtime: HerdrSlpRuntime,
  team: { generation: number; project_path: string; runtime_pane_id: string; team_id: string; workspace_id: string },
): Promise<string> {
  if (team.runtime_pane_id && await runtime.paneAlive(team.runtime_pane_id)) return team.runtime_pane_id;
  let paneId: string;
  try {
    paneId = await openRuntimePane(runtime.client, team, null);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new CliError(
      "RUNTIME_PANE_FAILED",
      `${team.team_id} g${team.generation} is RUNNING but its attention runtime pane did not open: ${message}; run maestro install (it links the maestro Herdr plugin), then repeat team start`,
      { generation: team.generation, teamId: team.team_id },
    );
  }
  recordRuntimePane(team.project_path, team.team_id, team.generation, paneId);
  return paneId;
}

// w696: team start creates the Lead's first item itself, so it owes that item
// the same d753 wake-up work add sends; without it the Lead pane holds only its
// acknowledgement and sits idle while the store says OPEN. A repeat start
// re-wakes an item still OPEN and stays quiet once the Lead has taken it.
async function pushInitialWorkNotice(
  projectPath: string,
  work: { assigned_to: string; id: string; objective: string; state: WorkState },
): Promise<void> {
  if (work.state !== "OPEN") return;
  await pushNotice(
    projectPath,
    "hub-supervisor",
    work.assigned_to,
    `${work.id} OPEN`,
    work.objective,
    `maestro status ${work.id}`,
  );
}

async function startTeam(
  context: PluginContext,
  runtime: HerdrSlpRuntime,
  projectInput: string,
  objective: string,
  // room d128: the seat models chosen at start, pinned for the generation's life.
  models: PackModels = {},
): Promise<CliResult> {
  if (!isRoom(context.store.database)) {
    throw new CliError("ROLE_FORBIDDEN", "team start is Hub Supervisor authority and must run from ~/maestro");
  }
  const resolvedProjectPath = resolve(projectInput);
  const projectStat = await stat(resolvedProjectPath).catch(() => null);
  if (!projectStat?.isDirectory()) {
    throw new CliError("NOT_FOUND", `project directory not found: ${resolvedProjectPath}`);
  }
  const projectLocation = resolveStoreLocation(resolvedProjectPath);
  const projectPath = realpathSync.native(projectLocation.root);
  const teamId = teamIdForProject(projectPath);
  const roomRoot = resolveStoreLocation(process.cwd()).root;
  const packPath = join(roomRoot, "SLP.md");
  if (!existsSync(packPath)) {
    throw new CliError("SLP_PACK_MISSING", `missing canonical Workspace Pack: ${packPath}`);
  }
  const hubPackBytes = new Uint8Array(await readFile(packPath));
  // The Lead and Peer panes run in the project, so the start turns the plugin on there.
  await updateEntry(join(projectPath, ".maestro", "config"), "slp", false);
  migrateRoom(context.store);
  const snapshotPath = join(projectPath, ".maestro", "SLP.md");
  let attachedRoom = false;
  const projectStore = new Store(projectLocation.path);
  try {
    migrateProject(projectStore);
    projectStore.database.query("ATTACH DATABASE ? AS slp_room").run(context.store.path);
    attachedRoom = true;
    projectStore.database.exec("PRAGMA busy_timeout = 300000");
    const hubPack = new TextDecoder().decode(hubPackBytes);
    const hubVersion = packVersion(hubPack);
    const hubDigest = createHash("sha256").update(hubPackBytes).digest("hex");
    const hubSeats = await resolveSeats(projectPath, models);
    const hubConfiguration: PackConfiguration = {
      ...(Object.keys(models).length > 0 ? { models } : {}),
      profileDigests: hubSeats.profileDigests,
    };
    await archivePack(roomRoot, hubPackBytes, hubDigest);
    const ownerToken = randomUUID();
    const deadline = Date.now() + 30_000;
    let reservation: StartReservation;
    while (true) {
      reservation = reserveStart(
        projectStore,
        {
          configuration: hubConfiguration,
          digest: hubDigest,
          models,
          objective,
          projectPath,
          teamId,
          version: hubVersion,
        },
        ownerToken,
      );
      if (reservation.kind !== "wait") break;
      if (Date.now() >= deadline) {
        throw new CliError(
          "TEAM_START_PENDING",
          `${teamId} is still being started by another process; inspect status and retry`,
        );
      }
      await Bun.sleep(50);
    }
    // room d128: the models this generation launches with, read from the row
    // it will use (pinned on a restore), printed once beside the phase lines
    // so a live check has something to read besides the pane.
    const launchModels = packConfiguration(reservation.row.configuration_json).models;
    if (launchModels) {
      const chosen = [
        launchModels.lead ? `lead model: ${launchModels.lead}` : null,
        launchModels.peer ? `peer model: ${launchModels.peer}` : null,
      ].filter((part) => part !== null);
      process.stderr.write(`${chosen.join("; ")}\n`);
    }

    if (reservation.kind === "running") {
      const running = reservation.row;
      try {
        const configuration = packConfiguration(running.configuration_json);
        if (!existsSync(snapshotPath)) {
          throw new CliError(
            "SLP_SNAPSHOT_MISSING",
            `running generation ${running.team_id}:g${running.generation} is missing ${snapshotPath}`,
          );
        }
        const snapshotBytes = new Uint8Array(await readFile(snapshotPath));
        const snapshotDigest = createHash("sha256").update(snapshotBytes).digest("hex");
        if (snapshotDigest !== running.pack_digest) {
          throw new CliError(
            "SLP_SNAPSHOT_CHANGED",
            `running generation ${running.team_id}:g${running.generation} must keep its pinned Workspace Pack`,
            { actual: snapshotDigest, expected: running.pack_digest },
          );
        }
        await requireSeatsUnchanged(configuration, running);
        await archivePack(roomRoot, snapshotBytes, running.pack_digest);
        const seats = await resolveSeats(projectPath, configuration.models);
        const plan = buildSlpTeamPlan({
          generation: running.generation,
          lead: seats.lead,
          projectPath,
          teamId: running.team_id,
        });
        const recorded = projectStore.database
          .query<{
            brief_digest: string;
            instance_id: string;
            pack_digest: string;
            pane_id: string;
            ready_challenge: string;
            role: SlpRole;
          }, [string, number]>(
            `SELECT role, pane_id, instance_id, pack_digest, brief_digest, ready_challenge
             FROM slp_local_roles
             WHERE team_id = ? AND generation = ? AND instance_id <> ''
               AND role = 'lead'`,
          )
          .all(running.team_id, running.generation);
        const existingInstances = Object.fromEntries(
          recorded.map((row) => [row.role, row.instance_id]),
        ) as Partial<Record<SlpRole, string>>;
        const acknowledged = new Map<SlpRole, SlpAcknowledgedRole>(
          recorded
            .filter((row) => row.pane_id !== "" && row.ready_challenge !== "")
            .map((row) => [row.role, {
              briefDigest: row.brief_digest,
              instanceId: row.instance_id,
              packDigest: row.pack_digest,
              paneId: row.pane_id,
              readyChallenge: row.ready_challenge,
            }]),
        );
        const started = await runtime.start(
          plan,
          roleContracts(
            running.team_id,
            running.generation,
            running.pack_digest,
            existingInstances,
          ),
          acknowledged,
        );
        try {
          const work = withImmediateTransaction(projectStore, () => {
            const current = projectStore.database
              .query<{ state: string }, [string, number]>(
                `SELECT state FROM slp_room.slp_teams WHERE team_id = ? AND generation = ?`,
              )
              .get(running.team_id, running.generation);
            if (current?.state !== "RUNNING") {
              throw new CliError(
                "INVALID_STATE",
                `${running.team_id}:g${running.generation} changed during runtime repair`,
              );
            }
            const now = new Date().toISOString();
            upsertStartedRoles(
              projectStore,
              "slp_local_roles",
              running.team_id,
              running.generation,
              started.roles,
              now,
            );
            upsertStartedRoles(
              projectStore,
              "slp_room.slp_team_roles",
              running.team_id,
              running.generation,
              started.roles,
              now,
            );
            refreshStartRuntime(projectStore, reservation.operation, ownerToken, started, now);
            const initial = initialWorkRow(projectStore, running.team_id, running.generation);
            if (!initial) {
              throw new CliError(
                "SLP_INITIAL_WORK_MISSING",
                `running generation ${running.team_id}:g${running.generation} has no initial work`,
              );
            }
            return initial;
          });
          const runtimePaneId = await ensureRuntimePane(
            runtime,
            {
              generation: running.generation,
              project_path: projectPath,
              runtime_pane_id: runtimePaneIdOf(projectStore, running.team_id, running.generation),
              team_id: running.team_id,
              workspace_id: started.workspaceId,
            },
          );
          await pushInitialWorkNotice(projectPath, work);
          context.sessions.record("team.start");
          return {
            data: {
              team: {
                generation: running.generation,
                packDigest: running.pack_digest,
                packVersion: running.pack_version,
                projectPath,
                roles: started.roles,
                runtimePaneId,
                state: "RUNNING",
                teamId: running.team_id,
                workspaceId: started.workspaceId,
              },
              work: toWork(work),
            },
            text:
              `${running.team_id} generation ${running.generation} running; ` +
              `${work.id} ${work.state} for ${work.assigned_to}`,
          };
        } catch (error) {
          await runtime.rollback(plan, {
            createdTabIds: started.createdTabIds,
            createdWorkspace: started.createdWorkspace,
            startedPaneIds: started.startedPaneIds,
            workspaceId: started.workspaceId,
          });
          throw error;
        }
      } finally {
        releaseRunningStartRepair(projectStore, reservation.operation, ownerToken);
      }
    }

    let operation = reservation.row;
    const previousSnapshot = existsSync(snapshotPath)
      ? new Uint8Array(await readFile(snapshotPath))
      : null;
    let runtimeReadyPersisted = operation.phase === "RUNTIME_READY";
    try {
      const archivedPath = join(roomRoot, ".maestro", "packs", `${operation.pack_digest}.md`);
      const packBytes = new Uint8Array(await readFile(archivedPath));
      const actualDigest = createHash("sha256").update(packBytes).digest("hex");
      if (actualDigest !== operation.pack_digest) {
        throw new CliError(
          "SLP_PACK_ARCHIVE_CORRUPT",
          `archived Workspace Pack ${archivedPath} does not match ${operation.pack_digest}`,
          { actual: actualDigest, expected: operation.pack_digest, path: archivedPath },
        );
      }
      const configuration = packConfiguration(operation.configuration_json);
      const seats = await resolveSeats(projectPath, configuration.models);
      const plan = buildSlpTeamPlan({
        generation: operation.generation,
        lead: seats.lead,
        projectPath,
        teamId: operation.team_id,
      });
      await mkdir(join(projectPath, ".maestro"), { recursive: true });
      await writeFile(snapshotPath, packBytes);

      let roles = lifecycleRuntimeRoles(operation);
      let started: SlpRuntimeStart | null = null;
      if (operation.phase === "RUNTIME_READY" && roles && operation.workspace_id) {
        const inspection = await runtime.inspect(plan, roles).catch(() => null);
        if (!inspection?.workspace || inspection.missingPanes.length > 0) roles = null;
      }
      if (!roles) {
        const existingInstances = Object.fromEntries(
          (lifecycleRuntimeRoles(operation) ?? []).map((role) => [role.role, role.instanceId]),
        ) as Partial<Record<SlpRole, string>>;
        started = await runtime.start(
          plan,
          roleContracts(
            operation.team_id,
            operation.generation,
            operation.pack_digest,
            existingInstances,
          ),
        );
        operation = recordStartRuntimeReady(projectStore, operation, ownerToken, started);
        runtimeReadyPersisted = true;
        roles = started.roles;
      }
      if (!roles) throw new CliError("RUNTIME_INCOMPLETE", "SLP roles were not recoverable");
      const work = finalizeStart(projectStore, operation, ownerToken, roles, context.store.path);
      const workspaceId = operation.workspace_id ?? started?.workspaceId ?? "";
      const runtimePaneId = await ensureRuntimePane(
        runtime,
        {
          generation: operation.generation,
          project_path: projectPath,
          runtime_pane_id: runtimePaneIdOf(projectStore, operation.team_id, operation.generation),
          team_id: operation.team_id,
          workspace_id: workspaceId,
        },
      );
      await pushInitialWorkNotice(projectPath, work);
      context.sessions.record("team.start");
      return {
        data: {
          team: {
            generation: operation.generation,
            packDigest: operation.pack_digest,
            packVersion: operation.pack_version,
            projectPath,
            roles,
            runtimePaneId,
            state: "RUNNING",
            teamId: operation.team_id,
            workspaceId,
          },
          work: toWork(work),
        },
        text:
          `${operation.team_id} generation ${operation.generation} running; ` +
          `${work.id} ${work.state} for ${work.assigned_to}`,
      };
    } catch (error) {
      try {
        releaseLifecycleOwner(projectStore, operation, ownerToken);
      } catch {}
      if (!runtimeReadyPersisted) await restoreSnapshot(snapshotPath, previousSnapshot);
      throw error;
    }
  } finally {
    if (attachedRoom) projectStore.database.exec("DETACH DATABASE slp_room");
    projectStore.close();
  }
}

interface ActiveLocalTeam {
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
function hubSupervisorTarget(team: Pick<ActiveLocalTeam, "supervisor_pane_id">): string {
  return team.supervisor_pane_id || "supervisor";
}

interface SlpActor {
  name: string;
  role: SlpRole;
  team: ActiveLocalTeam;
}

// room d117: the reviewer of the Lead's work is the Hub Supervisor, which
// holds no seat in the team and so is not an SlpActor. The reviewer
// operations - accept, rework, and the status read behind them - are widened
// to this type, which every SlpActor already satisfies; nothing else in the
// protocol accepts it, so a hub actor can never take, return or note work.
interface SlpReviewerActor {
  name: string;
  role: SlpRole | typeof hubReviewerName;
  team: ActiveLocalTeam;
}

interface SlpWorkRow {
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

function activeLocalTeam(context: PluginContext): ActiveLocalTeam | null {
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

function requireActiveOrLegacy(context: PluginContext): ActiveLocalTeam | null {
  const active = activeLocalTeam(context);
  if (!active && hasAdoptedLocalSlp(context)) {
    throw new CliError("NO_ACTIVE_TEAM", "no running SLP team is bound to this workspace");
  }
  return active;
}

function requireSlpActor(context: PluginContext, allowed: readonly SlpRole[]): SlpActor {
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

function requireSlpWork(context: PluginContext, actor: SlpReviewerActor, id: string): SlpWorkRow {
  const work = context.store.database
    .query<SlpWorkRow, [string, string, number]>(
      "SELECT * FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
    )
    .get(id, actor.team.team_id, actor.team.generation);
  if (!work) throw new CliError("NOT_FOUND", `SLP work not found: ${id}`);
  return work;
}

function requireRunningState(store: Store, team: ActiveLocalTeam): void {
  const state = store.database
    .query<{ state: string }, [string, number]>(
      "SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?",
    )
    .get(team.team_id, team.generation)?.state;
  if (state !== "RUNNING") {
    throw new CliError("NO_ACTIVE_TEAM", `no running SLP team in ${team.project_path}`);
  }
}

function requireRunningGeneration(store: Store, team: ActiveLocalTeam): void {
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

function workData(work: SlpWorkRow): Record<string, unknown> {
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

const hubReviewerName = "hub-supervisor";

function reviewerLabel(reviewer: WorkReviewer): string {
  return reviewer.kind === "hub" ? hubReviewerName : reviewer.role;
}

function expectedReviewer(
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

function requireWorkReviewer(
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

function stopSuffix(stop: { emergency: boolean; reason: string } | null): string {
  if (!stop || stop.emergency || stop.reason === "") return "";
  return ` (supervisor): ${stop.reason}`;
}

// d870/d873: the verdict reads as a question, not a finding, because from a
// read a downed Herdr looks the same as a dead team - and it names the one
// command that closes a real orphan by hand, which works today on a pane-less
// generation. Nothing here stops anything.
function orphanedSuffix(team: Record<string, unknown>): string {
  if (team.orphaned !== true) return "";
  return `; ORPHANED? no lead pane - a Herdr that is down reads the same from here;` +
    ` if it is real, close it with: maestro team stop ${String(team.teamId)} --emergency`;
}

function noticeSummary(body: string): string {
  const line = body.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > 160 ? `${line.slice(0, 157)}...` : line;
}

// d753/d760: the store is the truth; the pushed line is only the wake-up.
async function pushNotice(
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

function rolePaneName(
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
function reviewerPushTarget(
  context: PluginContext,
  actor: SlpActor,
  work: SlpWorkRow,
): string | null {
  const reviewer = expectedReviewer(context, actor, work);
  return reviewer.kind === "hub"
    ? hubSupervisorTarget(actor.team)
    : rolePaneName(context, actor, reviewer.role);
}

function readSlpWork(context: PluginContext, actor: SlpReviewerActor, id: string): SlpWorkRow {
  return requireSlpWork(context, actor, id);
}

function recordProjectActivity(
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

function requireWorkTransition(changes: number, id: string, operation: string): void {
  if (changes !== 1) {
    throw new CliError(
      "INVALID_STATE",
      `${id} changed before ${operation}; inspect its current status and retry`,
    );
  }
}

function normalizedPeerName(teamId: string, raw: string, existing: readonly string[]): string {
  if (existing.includes(raw)) return raw;
  const candidate = raw.startsWith("peer-") ? raw.slice("peer-".length) : raw;
  const normalized = candidate
    .toLowerCase()
    .replaceAll(/[^a-z0-9-]+/g, "-")
    .replaceAll(/^-|-$/g, "");
  if (!/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(normalized)) {
    throw new CliError("INVALID_PEER", "--to must name a Peer using letters, numbers, or hyphens");
  }
  const readable = normalized.slice(0, 20).replace(/-$/, "");
  const identity = createHash("sha256")
    .update(`${teamId}\0${normalized}`)
    .digest("hex")
    .slice(0, 6);
  return `peer-${readable}-${identity}`;
}

// w701 (d859): --acceptance and --blocked-by are declared in the shared work add
// flag table, so the parser accepts them inside a team and the SLP branch, which
// reads only --to, --model and --fresh, then drops them without a word. Refuse
// instead of persisting: SLP has no acceptance column and no gating concept, so a
// stored blocker nothing enforces would read as a gate that is not one. Both flags
// keep working unchanged on the Hub path below this branch.
function refuseDiscardedWorkAddOptions(invocation: CliInvocation): void {
  if (invocation.options.start !== undefined) {
    throw new CliError("INVALID_OPTION", "--start does not apply to SLP work add; the assignee takes the item with maestro work take <id>");
  }
  const discarded = ["acceptance", "blocked-by"].filter(
    (flag) => invocation.options[flag] !== undefined,
  );
  if (discarded.length === 0) return;
  throw new CliError(
    "INVALID_OPTION",
    `${discarded.map((flag) => `--${flag}`).join(" and ")} ` +
      `${discarded.length > 1 ? "are" : "is"} not recorded by SLP work add; ` +
      "write the acceptance condition and any ordering into the objective text",
  );
}

async function maybeHandleSlpWorkAdd(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  if (isRoom(context.store.database) && invocation.options.to !== undefined) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      "Hub Supervisor cannot assign team work directly; communicate through the Lead",
    );
  }
  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["lead"]);
  requireRunningGeneration(context.store, actor.team);
  refuseDiscardedWorkAddOptions(invocation);
  const objective = requiredPosition(invocation, 0, "work objective");
  const requestedTarget = stringOption(invocation, "to");
  // room d128: --model launches this one Peer with that model, over the
  // generation's --peer-model.
  const modelOption = stringOption(invocation, "model");
  if (modelOption !== undefined && modelOption.trim() === "") {
    throw new CliError("INVALID_OPTION", "--model needs a model name; the harness validates it, maestro passes it through");
  }
  const fresh = invocation.options.fresh === true;
  const roles = context.store.database
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
              brief_digest, ready_challenge, profile FROM slp_local_roles
       WHERE team_id = ? AND generation = ?`,
    )
    .all(actor.team.team_id, actor.team.generation);
  let assignee = roles.find((role) => role.role === "lead") ?? null;
  let createdPeerTab: string | null = null;
  let startedPeerPane: string | null = null;
  let plan: SlpTeamPlan | null = null;
  const runtime = new HerdrSlpRuntime();
  if (!requestedTarget) {
    // room d118 (w725): a Lead opens its own item with no --to. The row is the
    // one team start writes for Lead work - the Lead is its own assignee - so
    // no Peer pane is opened, no wake line goes out, and its reviewer is the
    // seat above exactly as for Lead work today.
    if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "the team has no Lead");
    if (modelOption !== undefined) {
      throw new CliError("INVALID_OPTION", "--model applies to Lead work add --to <peer>");
    }
    if (fresh) throw new CliError("INVALID_OPTION", "--fresh applies to Lead work add --to <peer>");
  } else {
    const peerName = normalizedPeerName(
      actor.team.team_id,
      requestedTarget,
      roles.filter((role) => role.role === "peer").map((role) => role.name),
    );
    const configuration = packConfiguration(actor.team.configuration_json);
    const snapshotPath = join(actor.team.project_path, ".maestro", "SLP.md");
    const snapshot = new Uint8Array(await readFile(snapshotPath));
    const digest = createHash("sha256").update(snapshot).digest("hex");
    if (digest !== actor.team.pack_digest) {
      throw new CliError(
        "SLP_SNAPSHOT_CHANGED",
        `running generation ${actor.team.team_id}:g${actor.team.generation} must keep its pinned Workspace Pack`,
      );
    }
    await requireSeatsUnchanged(configuration, actor.team);
    const peerSeat = await resolveSeat("peer", actor.team.project_path, resolveHomeDirectory());
    const knownPeer = roles.find((role) => role.name === peerName) ?? null;
    // room d128 (w728 rework): the launch line is the only carrier for the
    // model, and an acknowledged pane is never relaunched by work add - not
    // even by --fresh, whose /clear keeps the process. The role row does not
    // keep the launch model, so nothing could prove a repeated value matches
    // either; any --model on a reused pane is refused before a pane is
    // ensured and before any store write.
    if (modelOption !== undefined && knownPeer && knownPeer.ready_challenge !== "") {
      throw new CliError(
        "INVALID_OPTION",
        `--model rides the launch line only, and ${peerName} is already running in ${knownPeer.pane_id}; drop --model to reuse it as it is, or pick another Peer name to open a new pane with --model ${modelOption}`,
        { model: modelOption, pane: knownPeer.pane_id, peer: peerName },
      );
    }
    // Hub d101: a reset mid-item would drop the Peer's working context, so
    // --fresh is refused before anything is written while it holds ACTIVE work.
    if (fresh) {
      const held = context.store.database
        .query<{ id: string }, [string, number, string, string]>(
          `SELECT id FROM slp_work
           WHERE team_id = ? AND generation = ? AND state = 'ACTIVE'
             AND (owner = ? OR assigned_to = ?)
           ORDER BY id LIMIT 1`,
        )
        .get(actor.team.team_id, actor.team.generation, peerName, peerName);
      if (held) {
        throw new CliError(
          "PEER_ACTIVE",
          `${peerName} holds ${held.id} ACTIVE; --fresh resets a Peer pane only between items, wait for its return or add without --fresh`,
          { peer: peerName, work: held.id },
        );
      }
    }
    plan = await planForTeam(actor.team);
    const peerPlan: SlpRolePlan = {
      autocompact: peerSeat.config.autocompact ?? null,
      kind: peerSeat.config.harness,
      label: `slp:${actor.team.team_id}:g${actor.team.generation}:peer:${peerName}`,
      model: modelOption ?? configuration.models?.peer ?? null,
      name: peerName,
      profile: "peer",
      role: "peer",
    };
    const ensured = await runtime.ensurePeer(
      plan,
      peerPlan,
      roleContracts(
        actor.team.team_id,
        actor.team.generation,
        digest,
      ).get("peer") as SlpRoleContract,
      knownPeer && knownPeer.ready_challenge !== ""
        ? {
          briefDigest: knownPeer.brief_digest,
          instanceId: knownPeer.instance_id,
          packDigest: knownPeer.pack_digest,
          paneId: knownPeer.pane_id,
          readyChallenge: knownPeer.ready_challenge,
        }
        : null,
    );
    createdPeerTab = ensured.createdTabId;
    startedPeerPane = ensured.startedPaneId;
    let identity = ensured.role;
    // Hub d101: an acknowledged pane is reset and re-challenged before the
    // store write (d757 order: proven, then written); a reset that is not
    // proven refuses the whole add, nothing is written and no wake line goes
    // out (live g22 2026-09-05: an unproven reset went OPEN on stale context).
    if (fresh && ensured.reused) {
      const contract = roleContracts(
        actor.team.team_id,
        actor.team.generation,
        digest,
        { peer: identity.instanceId },
      ).get("peer") as SlpRoleContract;
      try {
        await runtime.resetPeer(plan, peerPlan, identity.paneId, contract);
      } catch (error) {
        throw new CliError(
          "PEER_RESET_FAILED",
          `could not reset ${peerName} in ${identity.paneId} before adding work: ${error instanceof Error ? error.message : String(error)}; nothing was added, retry --fresh or add without it`,
          { pane: identity.paneId, peer: peerName },
        );
      }
      identity = {
        ...identity,
        briefDigest: contract.briefDigest,
        readyChallenge: contract.readyChallenge,
      };
    }
    assignee = {
      name: identity.name,
      pane_id: identity.paneId,
      profile: identity.profile,
      role: identity.role,
      workspace_id: identity.workspaceId,
      instance_id: identity.instanceId,
      pack_digest: identity.packDigest,
      brief_digest: identity.briefDigest,
      ready_challenge: identity.readyChallenge,
    };
  }
  if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "work assignee is unavailable");
  const opensPeer = assignee.role === "peer";

  const now = new Date().toISOString();
  let attachedRoom = false;
  let id = "";
  try {
    if (opensPeer) {
      const roomStore = new Store(actor.team.room_store_path);
      try {
        migrateRoom(roomStore);
      } finally {
        roomStore.close();
      }
      context.store.database.query("ATTACH DATABASE ? AS slp_room").run(actor.team.room_store_path);
      attachedRoom = true;
    }
    context.store.database.exec("BEGIN IMMEDIATE");
    requireRunningGeneration(context.store, actor.team);
    if (opensPeer) {
      for (const table of ["slp_local_roles", "slp_room.slp_team_roles"]) {
        context.store.database
          .query(
            `INSERT INTO ${table}
              (team_id, generation, role, name, pane_id, workspace_id, instance_id,
               pack_digest, brief_digest, ready_challenge, profile, created_at)
             VALUES (?, ?, 'peer', ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(team_id, generation, name) DO UPDATE SET
               pane_id = excluded.pane_id,
               workspace_id = excluded.workspace_id,
               instance_id = excluded.instance_id,
               pack_digest = excluded.pack_digest,
               brief_digest = excluded.brief_digest,
               ready_challenge = excluded.ready_challenge,
               profile = excluded.profile`,
          )
          .run(
            actor.team.team_id,
            actor.team.generation,
            assignee.name,
            assignee.pane_id,
            assignee.workspace_id,
            assignee.instance_id,
            assignee.pack_digest,
            assignee.brief_digest,
            assignee.ready_challenge,
            assignee.profile,
            now,
          );
      }
    }
    id = context.store.nextCountedId("w", ["work", "slp_work", "slp_lifecycle_operations.work_id"]);
    context.store.database
      .query(
        `INSERT INTO slp_work
          (id, team_id, generation, objective, created_by, assigned_to, owner,
           state, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL, 'OPEN', ?, ?)`,
      )
      .run(
        id,
        actor.team.team_id,
        actor.team.generation,
        objective,
        actor.name,
        assignee.name,
        now,
        now,
      );
    recordProjectActivity(context, actor, "work.add", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    let cleanupError: unknown = null;
    if ((startedPeerPane || createdPeerTab) && plan) {
      try {
        if (startedPeerPane) await runtime.closeStartedPane(plan, startedPeerPane);
        else if (createdPeerTab) await runtime.closeCreatedTab(plan, createdPeerTab);
      } catch (caught) {
        cleanupError = caught;
      }
    }
    if (cleanupError) throw cleanupError;
    throw error;
  } finally {
    if (attachedRoom) context.store.database.exec("DETACH DATABASE slp_room");
  }
  context.sessions.record("work.add");
  // Live row 17 (g18): an acknowledged Peer pane never learned about its next
  // item and a fresh one sat idle after READY, so the assignee is woken here
  // in both cases, the same d753 line as return and accept. A Lead's own item
  // (room d118) has no counterpart to wake.
  if (assignee.name !== actor.name) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      assignee.name,
      `${id} OPEN`,
      objective,
      `maestro status ${id}`,
    );
  }
  const work = readSlpWork(context, actor, id);
  return {
    data: {
      role: {
        name: assignee.name,
        paneId: assignee.pane_id,
        profile: assignee.profile,
        role: assignee.role,
        workspaceId: assignee.workspace_id,
      },
      work: workData(work),
    },
    text: `${id} OPEN for ${assignee.name}`,
  };
}

async function maybeHandleSlpWorkNote(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  // room d117: the Hub grants rework on the Lead's work. It is a
  // reviewer here and nothing more - a Hub --blocked note would be the Hub
  // escalating to itself, so it is refused rather than quietly recorded.
  // w736 F1: the plain note joins the grant. A reviewer answer is not always a
  // ruling - a fact, a commit hash, a pointer to a decision has no
  // `maestro decide` shape - and the room's plain note used to fall through to
  // the non-SLP work table, whose "work not found; run: maestro work list"
  // sent the reader to the wrong store. The room's OWN work items keep that
  // table and that verb; only an id it does not hold is resolved as SLP work,
  // so the failure names the SLP resolver.
  if (isRoom(context.store.database)) {
    const id = requiredPosition(invocation, 0, "work id");
    const flags = [
      ...(invocation.options.blocked === true ? ["--blocked"] : []),
      ...(invocation.options.rework === true ? ["--rework"] : []),
      ...(invocation.options.owner === true ? ["--owner"] : []),
    ];
    if (flags.length > 1) {
      throw new CliError("INVALID_OPTION", `${flags.join(" and ")} are separate notes`);
    }
    if (flags.length === 0 && roomHoldsLocalWork(context.store, id)) {
      // Two id spaces meet on this verb, and both are live: the room's own
      // `work` sequence and every team's. They do not overlap today, and the
      // room's d118/d125 collision in `decisions` is what one looks like when
      // it happens, so when both hold the id neither wins silently - the Hub
      // qualifies the team's item the way it already qualifies an ambiguous
      // one (w736 F1).
      let team: string | null = null;
      try {
        team = resolveHubWorkTarget(context, id, "read").teamId;
      } catch {
        team = null;
      }
      if (team === null) return null;
      throw new CliError(
        "AMBIGUOUS_WORK",
        `${id} names both a Hub work item and SLP work in ${team}; qualify the team's item as ${team}:${id}`,
        { candidates: [id, `${team}:${id}`] },
      );
    }
    if (invocation.options.blocked === true) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "the Hub Supervisor has no seat above it inside SLP, so --blocked from the room would be the" +
          " Hub escalating to itself (room d117); a question for the owner goes to the owner, and an" +
          " answer that settles a fork is maestro decide",
      );
    }
    if (invocation.options.owner === true) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "--owner is a seat's record of an owner walk-in into its own pane before it acts (room d123);" +
          " an owner instruction in the Hub pane is recorded as maestro decide --scope owner",
      );
    }
    const bound = bindHubReviewer(context, id);
    try {
      return await noteWorkAs(bound.context, bound.actor, invocation, bound.workId);
    } finally {
      bound.close();
    }
  }
  if (!requireActiveOrLegacy(context)) return null;
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  return noteWorkAs(context, actor, invocation);
}

// The Hub room keeps its own non-SLP work items, whose ids run a separate
// sequence from any team's. A plain note on one of those is still the plain
// verb; anything else is SLP work and answers from the SLP resolver (w736 F1).
function roomHoldsLocalWork(store: Store, id: string): boolean {
  if (!tableExists(store, "work")) return false;
  return store.database
    .query<{ present: number }, [string]>("SELECT 1 AS present FROM work WHERE id = ?")
    .get(id) !== null;
}

// room d117: reviewer-shaped like acceptWorkAs, so the Hub's --rework grant is
// the same grant the Lead records, not a parallel implementation.
async function noteWorkAs(
  context: PluginContext,
  actor: SlpReviewerActor,
  invocation: CliInvocation,
  // The Hub may address work as `<team-id>:<work-id>`; seats always pass none.
  resolvedId?: string,
): Promise<CliResult> {
  requireRunningGeneration(context.store, actor.team);
  const id = resolvedId ?? requiredPosition(invocation, 0, "work id");
  const body = requiredPosition(invocation, 1, "note body");
  const work = requireSlpWork(context, actor, id);
  const rework = invocation.options.rework === true;
  const blocked = invocation.options.blocked === true;
  // room d123: --owner is provenance, not a state. It marks the note as an
  // owner walk-in recorded by the seat before acting, so the seat above can
  // tell an owner instruction from the seat changing course on its own.
  const owner = invocation.options.owner === true;
  const separate = [
    ...(blocked ? ["--blocked"] : []),
    ...(rework ? ["--rework"] : []),
    ...(owner ? ["--owner"] : []),
  ];
  if (separate.length > 1) {
    throw new CliError("INVALID_OPTION", `${separate.join(" and ")} are separate notes`);
  }
  if (actor.role === "peer" && work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} may note only its assigned work`);
  }
  if (rework) {
    requireWorkReviewer(context, actor, work, "grant rework");
    if (work.state !== "RETURNED") {
      throw new CliError(
        "INVALID_STATE",
        `${id} must be RETURNED before its reviewer grants rework`,
      );
    }
  }
  const now = new Date().toISOString();
  const flag = blocked ? "blocked" : owner ? "owner" : null;
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const current = requireSlpWork(context, actor, id);
    if (rework) {
      requireWorkReviewer(context, actor, current, "grant rework");
      if (current.state !== "RETURNED" || current.return_revision <= 0) {
        throw new CliError(
          "INVALID_STATE",
          `${id} changed before its reviewer could grant rework`,
        );
      }
      const existing = context.store.database
        .query<{ present: number }, [string, number]>(
          `SELECT 1 AS present FROM slp_rework_grants
           WHERE work_id = ? AND return_revision = ?`,
        )
        .get(id, current.return_revision);
      if (existing) {
        throw new CliError(
          "REWORK_ALREADY_GRANTED",
          `${id} return revision ${current.return_revision} already has a rework grant`,
        );
      }
      context.store.database
        .query(
          `INSERT INTO slp_rework_grants
            (work_id, return_revision, reviewer, granted_at)
           VALUES (?, ?, ?, ?)`,
        )
        .run(id, current.return_revision, actor.name, now);
      markLessonCandidate(
        context.store,
        id,
        `rework note on revision ${current.return_revision}`,
        now,
      );
    }
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, flag, created_at)
         VALUES (?, 'NOTE', ?, ?, ?, ?)`,
      )
      .run(id, actor.name, body, flag, now);
    recordProjectActivity(context, actor, "work.note", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.note");
  if (rework) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      work.assigned_to,
      `${id} RETURNED`,
      `rework granted: ${body}`,
      `maestro status ${id}`,
    );
  }
  // w736 F1: a seat's plain note wakes nobody - the shared contract is explicit
  // that an instruction whose value expires travels on a waking operation and
  // never on a plain note. The Hub is not a seat in the team: it holds no
  // `work add`, no pane, and no other line down, so its plain note is the one
  // it has and it is delivered. The exception is exactly one seat wide.
  if (!rework && actor.role === hubReviewerName) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      work.assigned_to,
      `${id} NOTE`,
      body,
      `maestro status ${id}`,
    );
  }
  // d761: the blocked note wakes the seat above the actor, never the reviewer
  // of the item, because the one who is stuck is the one escalating; the seat
  // above the Lead is the Hub (room d117).
  // room d123: the owner walk-in rides the same path with its own subject; it
  // is one line up for the record, never a stall and never the blocked flag.
  if (blocked || owner) {
    await pushOneSeatUp(context, actor, id, blocked ? "BLOCKED" : "OWNER", body);
  }
  const kind = rework ? "rework grant" : blocked ? "blocked note" : owner ? "owner note" : "note";
  return {
    data: {
      note: { actor: actor.name, body, createdAt: now, flag, rework },
      work: workData(readSlpWork(context, actor, id)),
    },
    text: `${id} ${kind} by ${actor.name}: ${body}`,
  };
}

// The one upward push a seat has: Peer to Lead, and the Lead to the Hub pane
// (room d117). The Hub's line carries the team and points at the room's own
// status, because the Hub cannot read a team item by bare id.
async function pushOneSeatUp(
  context: PluginContext,
  actor: SlpReviewerActor,
  id: string,
  subject: "BLOCKED" | "OWNER",
  body: string,
): Promise<void> {
  // The Hub reviewer never notes --blocked or --owner (refused at the room
  // path), and it has no seat above to push to.
  if (actor.role === hubReviewerName) return;
  const toHub = actor.role === "lead";
  await pushNotice(
    actor.team.project_path,
    actor.role,
    toHub ? hubSupervisorTarget(actor.team) : rolePaneName(context, actor, "lead"),
    `${id} ${subject}`,
    toHub ? `${noticeSummary(body)} in ${actor.team.team_id} g${actor.team.generation}` : body,
    toHub ? "maestro status" : `maestro status ${id}`,
  );
}

// w740 (d898): opening a Full-tier bundle on an item is the store's signal
// that a design phase has begun, and it reaches the reviewer above the one
// designing - the Hub for the Lead, the Lead for a Peer.
// Every failure here is silent: a bundle open outside a running team, from a
// pane with no role, or on an id no SLP item holds, is an ordinary bundle open.
async function pushDesignOpened(
  context: PluginContext,
  bundleId: string,
  workIds: readonly string[],
): Promise<void> {
  if (workIds.length === 0) return;
  let actor: SlpActor;
  try {
    actor = requireSlpActor(context, ["lead", "peer"]);
  } catch {
    return;
  }
  for (const workId of workIds) {
    const work = context.store.database
      .query<SlpWorkRow, [string, string, number]>(
        "SELECT * FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
      )
      .get(workId, actor.team.team_id, actor.team.generation);
    if (!work) continue;
    const toHub = expectedReviewer(context, actor, work).kind === "hub";
    await pushNotice(
      actor.team.project_path,
      actor.role,
      reviewerPushTarget(context, actor, work),
      `${workId} DESIGN`,
      toHub
        ? `design bundle ${bundleId} in ${actor.team.team_id} g${actor.team.generation}`
        : `design bundle ${bundleId}`,
      toHub ? "maestro status" : `maestro status ${workId}`,
    );
  }
}

// w740 (room d154, d899): decide is the fifth waking operation. A ruling on an
// item is an instruction to the seat holding it, so the line goes to the
// assignee and not to the reviewer; that is what carries the Hub's order down
// to the Lead, which d154 relies on and no code sent before. A ruling
// with no item, or one a seat makes on its own item, wakes nobody.
async function pushDecision(
  store: Store,
  projectPath: string,
  actor: SlpRole | typeof hubReviewerName,
  actorName: string,
  workId: string,
  choice: string,
): Promise<void> {
  if (!tableExists(store, "slp_work")) return;
  const assignee = store.database
    .query<{ assigned_to: string }, [string]>("SELECT assigned_to FROM slp_work WHERE id = ?")
    .get(workId)?.assigned_to;
  if (!assignee || assignee === actorName) return;
  await pushNotice(projectPath, actor, assignee, `${workId} DECIDED`, choice, `maestro status ${workId}`);
}

// The Hub rules from the room store, which holds no slp_work; the item lives in
// the project store, so the assignee is read there, read-only, the way the
// lesson-candidate mark is written there.
async function pushHubDecision(target: HubWorkTarget, choice: string): Promise<void> {
  let store: Store;
  try {
    store = new Store(resolveStoreLocation(target.projectPath).path, { readonly: true });
  } catch {
    return;
  }
  try {
    await pushDecision(store, target.projectPath, hubReviewerName, hubReviewerName, target.workId, choice);
  } finally {
    store.close();
  }
}

function takeWork(context: PluginContext, id: string): CliResult {
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  if (work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${id} is assigned to ${work.assigned_to}`, {
      actor: actor.name,
      assignedTo: work.assigned_to,
    });
  }
  if (work.state !== "OPEN" && work.state !== "RETURNED") {
    throw new CliError("INVALID_STATE", `${id} must be OPEN or RETURNED before work take`);
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const current = requireSlpWork(context, actor, id);
    if (current.assigned_to !== actor.name) {
      throw new CliError("ROLE_FORBIDDEN", `${id} is assigned to ${current.assigned_to}`);
    }
    if (current.state !== "OPEN" && current.state !== "RETURNED") {
      throw new CliError("INVALID_STATE", `${id} changed before work take`);
    }
    if (current.state === "RETURNED") {
      const grant = context.store.database
        .query<{ consumed_at: string | null }, [string, number, string]>(
          `SELECT consumed_at FROM slp_rework_grants
           WHERE work_id = ? AND return_revision = ? AND reviewer <> ?`,
        )
        .get(id, current.return_revision, actor.name);
      if (!grant || grant.consumed_at !== null) {
        throw new CliError(
          "REWORK_REQUIRED",
          `${id} return revision ${current.return_revision} requires an unused grant from its reviewer`,
        );
      }
    }
    const transition = context.store.database
      .query(
        `UPDATE slp_work
         SET state = 'ACTIVE', owner = ?, current_return = NULL, updated_at = ?
         WHERE id = ? AND state = ?`,
      )
      .run(actor.name, now, id, current.state);
    requireWorkTransition(transition.changes, id, "work take");
    if (current.state === "RETURNED") {
      const consumed = context.store.database
        .query(
          `UPDATE slp_rework_grants SET consumed_at = ?
           WHERE work_id = ? AND return_revision = ? AND consumed_at IS NULL`,
        )
        .run(now, id, current.return_revision);
      requireWorkTransition(consumed.changes, id, "rework grant consumption");
    }
    recordProjectActivity(context, actor, "work.take", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.take");
  const current = readSlpWork(context, actor, id);
  return { data: { work: workData(current) }, text: `${id} ACTIVE by ${actor.name}` };
}

async function returnWork(context: PluginContext, id: string, body: string): Promise<CliResult> {
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead", "peer"]);
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  if (work.state !== "ACTIVE" || work.owner !== actor.name) {
    throw new CliError(
      "INVALID_STATE",
      `${id} must be ACTIVE and owned by ${actor.name} before work return`,
    );
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const transition = context.store.database
      .query(
        `UPDATE slp_work
         SET state = 'RETURNED', owner = NULL, current_return = ?,
             return_revision = return_revision + 1, updated_at = ?
         WHERE id = ? AND state = 'ACTIVE' AND owner = ?`,
      )
      .run(body, now, id, actor.name);
    requireWorkTransition(transition.changes, id, "work return");
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, created_at)
         VALUES (?, 'RETURN', ?, ?, ?)`,
      )
      .run(id, actor.name, body, now);
    recordProjectActivity(context, actor, "work.return", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.return");
  const current = readSlpWork(context, actor, id);
  await pushNotice(
    actor.team.project_path,
    actor.role,
    reviewerPushTarget(context, actor, current),
    `${id} RETURNED`,
    body,
    `maestro status ${id}`,
  );
  return { data: { work: workData(current) }, text: `${id} RETURNED by ${actor.name}` };
}

async function acceptWork(context: PluginContext, id: string, outcome: string): Promise<CliResult> {
  // room d117: from the Hub room this is the reviewer boundary of the Lead's
  // work; from a team workspace it is the seat boundary it has always been.
  if (isRoom(context.store.database)) {
    const bound = bindHubReviewer(context, id);
    try {
      return await acceptWorkAs(bound.context, bound.actor, bound.workId, outcome);
    } finally {
      bound.close();
    }
  }
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["lead"]);
  return acceptWorkAs(context, actor, id, outcome);
}

// room d117: the body below is reviewer-shaped rather than seat-shaped, so the
// Hub Supervisor runs exactly the same transition, entry and activity row the
// Lead does - one accept path, two callers, no second code path to drift.
// `context.store` is the PROJECT store in both cases.
async function acceptWorkAs(
  context: PluginContext,
  actor: SlpReviewerActor,
  id: string,
  outcome: string,
): Promise<CliResult> {
  requireRunningGeneration(context.store, actor.team);
  const work = requireSlpWork(context, actor, id);
  requireWorkReviewer(context, actor, work, "accept");
  if (outcome !== "accepted" && outcome !== "cancelled") {
    throw new CliError("INVALID_VALUE", "--outcome must be accepted or cancelled");
  }
  if (outcome === "cancelled" && work.state === "ACTIVE") {
    throw new CliError(
      "INVALID_STATE",
      `${id} is ACTIVE; its assignee must work return before cancellation`,
    );
  }
  if (
    (outcome === "accepted" && work.state !== "RETURNED") ||
    (outcome === "cancelled" && work.state !== "OPEN" && work.state !== "RETURNED")
  ) {
    throw new CliError(
      "INVALID_STATE",
      `${id} cannot be accepted with outcome ${outcome} from ${work.state}`,
    );
  }
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningGeneration(context.store, actor.team);
    const transition = outcome === "accepted"
      ? context.store.database
        .query(
          `UPDATE slp_work
           SET state = 'DONE', acceptance_outcome = ?, accepted_by = ?, updated_at = ?
           WHERE id = ? AND state = 'RETURNED'`,
        )
        .run(outcome, actor.name, now, id)
      : context.store.database
        .query(
          `UPDATE slp_work
           SET state = 'DONE', acceptance_outcome = ?, accepted_by = ?, updated_at = ?
           WHERE id = ? AND state IN ('OPEN', 'RETURNED')`,
        )
        .run(outcome, actor.name, now, id);
    requireWorkTransition(transition.changes, id, "work accept");
    context.store.database
      .query(
        `INSERT INTO slp_work_entries (work_id, kind, actor, body, created_at)
         VALUES (?, 'ACCEPTANCE', ?, ?, ?)`,
      )
      .run(id, actor.name, outcome, now);
    recordProjectActivity(context, actor, "work.accept", id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  context.sessions.record("work.accept");
  const current = readSlpWork(context, actor, id);
  await pushNotice(
    actor.team.project_path,
    actor.role,
    current.assigned_to,
    `${id} DONE`,
    outcome,
    `maestro status ${id}`,
  );
  if (current.created_by === "hub-supervisor") {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      hubSupervisorTarget(actor.team),
      `${id} DONE`,
      `${outcome} in ${actor.team.team_id} g${actor.team.generation}`,
      "maestro status",
    );
  }
  return {
    data: { work: workData(current) },
    text: `${id} DONE (${outcome}) by ${actor.name}`,
  };
}

interface HubWorkTarget {
  generation: number;
  projectPath: string;
  teamId: string;
  workId: string;
}

// w736 F3: a read reaches a stopped generation, a write does not. Every item
// the Hub accepted becomes unreadable the moment the team stops if the
// resolver only ever looks at RUNNING teams, and "SLP work not found from Hub"
// is the wrong sentence for a record that exists and is finished. Writing to a
// stopped generation stays refused - the seats are gone and the store is
// closed - but the refusal now names the state instead of denying the id.
type HubWorkAccess = "read" | "write";

interface HubTeamRow {
  generation: number;
  project_path: string;
  state: string;
  team_id: string;
}

// `tolerant` is the archive pass: the room remembers every generation it ever
// ran and some of those checkouts have since been deleted or moved, so a store
// that will not open there is skipped rather than failing the whole read. The
// RUNNING pass keeps the loud failure, because a live team whose project is
// unreachable is a fault worth stopping on.
function hubWorkMatches(
  teams: HubTeamRow[],
  workId: string,
  tolerant = false,
): HubWorkTarget[] {
  const matches: HubWorkTarget[] = [];
  for (const team of teams) {
    let projectStore: Store;
    try {
      projectStore = new Store(resolveStoreLocation(team.project_path).path, { readonly: true });
    } catch (error) {
      if (tolerant) continue;
      throw new CliError(
        "PROJECT_UNAVAILABLE",
        `cannot inspect SLP work in ${team.project_path}`,
        { cause: error instanceof Error ? error.message : String(error) },
      );
    }
    try {
      if (!tableExists(projectStore, "slp_work")) continue;
      const present = projectStore.database
        .query<{ present: number }, [string, string, number]>(
          `SELECT 1 AS present FROM slp_work
           WHERE id = ? AND team_id = ? AND generation = ?`,
        )
        .get(workId, team.team_id, team.generation);
      if (present) {
        matches.push({
          generation: team.generation,
          projectPath: team.project_path,
          teamId: team.team_id,
          workId,
        });
      }
    } finally {
      projectStore.close();
    }
  }
  return matches;
}

function oneHubWorkMatch(matches: HubWorkTarget[], workId: string): HubWorkTarget {
  // Newest generation first, so a team that reused an id across generations
  // answers with the latest; two different teams are still ambiguous and say so.
  const ordered = [...matches].sort((left, right) => right.generation - left.generation);
  const teams = new Set(ordered.map((match) => match.teamId));
  if (teams.size > 1) {
    throw new CliError(
      "AMBIGUOUS_WORK",
      `${workId} exists in multiple teams; qualify it as <team-id>:${workId}`,
      { candidates: ordered.map((match) => `${match.teamId}:${match.workId}`) },
    );
  }
  return ordered[0] as HubWorkTarget;
}

function resolveHubWorkTarget(
  context: PluginContext,
  reference: string,
  access: HubWorkAccess = "write",
): HubWorkTarget {
  migrateRoom(context.store);
  const separator = reference.indexOf(":");
  const qualifiedTeam = separator > 0 ? reference.slice(0, separator) : null;
  const workId = separator > 0 ? reference.slice(separator + 1) : reference;
  if (!workId) throw new CliError("INVALID_WORK_ID", "--work requires a work id");
  const all = context.store.database
    .query<HubTeamRow, []>(
      `SELECT team_id, generation, project_path, state FROM slp_teams
       ORDER BY project_path, generation`,
    )
    .all()
    .filter((team) => qualifiedTeam === null || team.team_id === qualifiedTeam);
  const running = all.filter((team) => team.state === "RUNNING");
  const matches = hubWorkMatches(running, workId);
  if (matches.length > 0) return oneHubWorkMatch(matches, workId);
  const closed = all.filter((team) => team.state !== "RUNNING");
  const archived = closed.length > 0 ? hubWorkMatches(closed, workId, true) : [];
  if (archived.length === 0) {
    throw new CliError("NOT_FOUND", `SLP work not found from Hub: ${reference}`);
  }
  const found = oneHubWorkMatch(archived, workId);
  if (access === "read") return found;
  const state = all.find(
    (team) => team.team_id === found.teamId && team.generation === found.generation,
  )?.state ?? "STOPPED";
  throw new CliError(
    "NO_ACTIVE_TEAM",
    `${found.teamId}:g${found.generation} is ${state}; a stopped generation's work is readable` +
      ` (maestro status ${workId}) and no longer writable`,
    { generation: found.generation, state, team: found.teamId, work: workId },
  );
}

// room d117: the Hub Supervisor reviews the Lead's work from ~/maestro. It has
// no pane in the team, so it cannot go through requireSlpActor; instead the
// work id is resolved to its team, the PROJECT store is opened, and a reviewer
// actor named `hub-supervisor` is handed to the same accept and rework bodies
// the seats use.
interface HubReviewerBinding {
  actor: SlpReviewerActor;
  close: () => void;
  context: PluginContext;
  // The unqualified id: the Hub may address work as `<team-id>:<work-id>`.
  workId: string;
}

function bindHubReviewer(
  context: PluginContext,
  reference: string,
  access: HubWorkAccess = "write",
): HubReviewerBinding {
  const target = resolveHubWorkTarget(context, reference, access);
  const projectStore = new Store(resolveStoreLocation(target.projectPath).path);
  try {
    // The Hub writes this store while the team's own seats are live in it;
    // without this a concurrent seat write fails the review outright rather
    // than waiting, which is the guard team start already sets for itself.
    projectStore.database.exec("PRAGMA busy_timeout = 300000");
    migrateProject(projectStore);
    // w736 F3: a read binds a stopped generation too; only the writes need it
    // live, and resolveHubWorkTarget has already refused those by name.
    const team = projectStore.database
      .query<ActiveLocalTeam, [string, number]>(
        `SELECT team_id, generation, room_store_path, project_path,
                configuration_json, pack_digest, workspace_id, runtime_pane_id,
                supervisor_pane_id
         FROM slp_local_teams
         WHERE team_id = ? AND generation = ?` +
          (access === "read" ? "" : " AND state = 'RUNNING'"),
      )
      .get(target.teamId, target.generation);
    if (!team) {
      throw new CliError(
        "NO_ACTIVE_TEAM",
        `${target.teamId}:g${target.generation} is no longer running`,
      );
    }
    return {
      actor: { name: hubReviewerName, role: hubReviewerName, team },
      close: () => projectStore.close(),
      context: { ...context, store: projectStore },
      workId: target.workId,
    };
  } catch (error) {
    projectStore.close();
    throw error;
  }
}

async function decideHubWork(
  context: PluginContext,
  input: {
    choice: string;
    replaces: string | null;
    scope: string;
    target: HubWorkTarget;
    why: string;
  },
): Promise<CliResult> {
  let transactionOpen = false;
  let id = "";
  const now = new Date().toISOString();
  try {
    context.store.database.exec("BEGIN IMMEDIATE");
    transactionOpen = true;
    const running = context.store.database
      .query<{ present: number }, [string, number]>(
        `SELECT 1 AS present FROM slp_teams
         WHERE team_id = ? AND generation = ? AND state = 'RUNNING'`,
      )
      .get(input.target.teamId, input.target.generation);
    if (!running) {
      throw new CliError(
        "NO_ACTIVE_TEAM",
        `${input.target.teamId}:g${input.target.generation} is no longer running`,
      );
    }
    if (input.replaces) {
      const prior = context.store.database
        .query<{ scope: string }, [string]>(
          "SELECT scope FROM slp_decisions WHERE id = ?",
        )
        .get(input.replaces);
      if (!prior) throw new CliError("NOT_FOUND", `decision to replace not found: ${input.replaces}`);
      if (prior.scope !== input.scope) {
        throw new CliError("INVALID_SCOPE", `${input.replaces} has scope ${prior.scope}, not ${input.scope}`);
      }
    }
    id = context.store.nextCountedId("d", ["decisions", "slp_decisions"]);
    const insertDecision = context.store.database.query(
      `INSERT INTO slp_decisions
        (id, team_id, generation, choice, why, scope, work_id, replaces_id,
         actor, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'hub-supervisor', ?)`,
    );
    const values = [
      id,
      input.target.teamId,
      input.target.generation,
      input.choice,
      input.why,
      input.scope,
      input.target.workId,
      input.replaces,
      now,
    ] as const;
    insertDecision.run(...values);
    if (input.replaces) clearProvisional(context.store, input.replaces);
    context.store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, 'hub-supervisor', 'decide', 'decision', ?, ?)`,
      )
      .run(input.target.teamId, input.target.generation, id, now);
    context.store.database.exec("COMMIT");
    transactionOpen = false;
  } catch (error) {
    if (transactionOpen) {
      try {
        context.store.database.exec("ROLLBACK");
      } catch {}
    }
    throw error;
  }
  // room d129 point 4: the Hub's ruling lands in the Hub store, but the item it
  // corrects lives in the project store, so the mark is written there. A store
  // that cannot be opened leaves the ruling committed and the mark unwritten
  // rather than failing a decision that is already recorded; the mark is a
  // reminder, never the ruling.
  if (input.replaces) {
    try {
      const projectStore = new Store(resolveStoreLocation(input.target.projectPath).path);
      try {
        markLessonCandidate(projectStore, input.target.workId, `decision ${id}`, now);
      } finally {
        projectStore.close();
      }
    } catch (error) {
      process.stderr.write(
        `lesson candidate not marked on ${input.target.workId}: ${
          error instanceof Error ? error.message : String(error)
        }\n`,
      );
    }
  }
  await pushHubDecision(input.target, input.choice);
  context.sessions.record("decide");
  const decision = {
    actor: "hub-supervisor",
    choice: input.choice,
    createdAt: now,
    id,
    replaces: input.replaces,
    scope: input.scope,
    why: input.why,
    workId: input.target.workId,
  };
  return {
    data: { decision },
    text: `${id} [${input.scope}] ${input.choice}`,
  };
}

// room d125: a decision that replaces a provisional ruling settles it; the
// superseded row keeps its text and loses only the flag. Runs inside the
// caller's transaction.
function clearProvisional(store: Store, id: string): void {
  if (!store.hasColumn("slp_decisions", "provisional")) return;
  store.database.query("UPDATE slp_decisions SET provisional = 0 WHERE id = ?").run(id);
}

async function decide(context: PluginContext, invocation: CliInvocation): Promise<CliResult> {
  const choice = requiredPosition(invocation, 0, "choice");
  const why = stringOption(invocation, "why")?.trim();
  if (!why) throw new CliError("MISSING_ARGUMENT", "decide requires --why <reason>");
  const local = activeLocalTeam(context);
  let actor: string;
  let actorRole: SlpRole = "lead";
  let teamId: string;
  let generation: number;
  let defaultScope: "technical" | "owner";
  let allowedScopes: readonly string[];
  if (local) {
    const role = requireSlpActor(context, ["lead"]);
    requireRunningGeneration(context.store, role.team);
    actor = role.name;
    actorRole = role.role;
    teamId = role.team.team_id;
    generation = role.team.generation;
    defaultScope = "technical";
    allowedScopes = [defaultScope];
  } else if (isRoom(context.store.database)) {
    actor = "hub-supervisor";
    teamId = "hub";
    generation = 0;
    defaultScope = "owner";
    allowedScopes = ["owner", "cross-team"];
  } else {
    throw new CliError("NO_ACTIVE_TEAM", "decide requires a Hub or running team context");
  }
  const scope = stringOption(invocation, "scope") ?? defaultScope;
  if (!allowedScopes.includes(scope)) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      `${actor} cannot decide scope ${scope}; allowed: ${allowedScopes.join(", ")}`,
    );
  }
  const workId = stringOption(invocation, "work") ?? null;
  const replaces = stringOption(invocation, "replaces") ?? null;
  if (workId && !local) {
    return await decideHubWork(context, {
      choice,
      replaces,
      scope,
      target: resolveHubWorkTarget(context, workId),
      why,
    });
  }
  if (workId && local) {
    const present = context.store.database
      .query<{ present: number }, [string, string, number]>(
        "SELECT 1 AS present FROM slp_work WHERE id = ? AND team_id = ? AND generation = ?",
      )
      .get(workId, teamId, generation);
    if (!present) throw new CliError("NOT_FOUND", `SLP work not found: ${workId}`);
  }
  if (replaces) {
    if (!tableExists(context.store, "slp_decisions")) {
      throw new CliError("NOT_FOUND", `decision to replace not found: ${replaces}`);
    }
    const prior = context.store.database
      .query<{ scope: string }, [string, string, number]>(
        `SELECT scope FROM slp_decisions
         WHERE id = ? AND team_id = ? AND generation = ?`,
      )
      .get(replaces, teamId, generation);
    if (!prior) throw new CliError("NOT_FOUND", `decision to replace not found: ${replaces}`);
    if (prior.scope !== scope) {
      throw new CliError("INVALID_SCOPE", `${replaces} has scope ${prior.scope}, not ${scope}`);
    }
  }
  if (!local) migrateRoom(context.store);
  let id = "";
  const now = new Date().toISOString();
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    if (local) requireRunningGeneration(context.store, local);
    id = context.store.nextCountedId("d", ["decisions", "slp_decisions"]);
    context.store.database
      .query(
        `INSERT INTO slp_decisions
          (id, team_id, generation, choice, why, scope, work_id, replaces_id,
           actor, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(id, teamId, generation, choice, why, scope, workId, replaces, actor, now);
    if (replaces && workId && local) {
      markLessonCandidate(context.store, workId, `decision ${id}`, now);
    }
    if (replaces && !local) clearProvisional(context.store, replaces);
    context.store.database
      .query(
        `INSERT INTO slp_activity
          (team_id, generation, actor, operation, target_type, target_id, created_at)
         VALUES (?, ?, ?, 'decide', 'decision', ?, ?)`,
      )
      .run(teamId, generation, actor, id, now);
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  if (local && workId) {
    await pushDecision(context.store, local.project_path, actorRole, actor, workId, choice);
  }
  context.sessions.record("decide");
  const decision = { actor, choice, createdAt: now, id, replaces, scope, why, workId };
  return { data: { decision }, text: `${id} [${scope}] ${choice}` };
}

function stopPlan(team: ActiveLocalTeam): Promise<SlpTeamPlan> {
  return planForTeam(team);
}

function stopRoles(store: Store, team: ActiveLocalTeam): SlpRuntimeRole[] {
  return roleRows(store, team.team_id, team.generation, true).map((role) => ({
    briefDigest: role.brief_digest,
    instanceId: role.instance_id,
    name: role.name,
    packDigest: role.pack_digest,
    paneId: role.pane_id,
    profile: role.profile,
    readyChallenge: role.ready_challenge,
    role: role.role,
    workspaceId: role.workspace_id,
  }));
}

function unfinishedWork(store: Store, team: ActiveLocalTeam): Array<{ id: string; state: WorkState }> {
  return store.database
    .query<{ id: string; state: WorkState }, [string, number]>(
      `SELECT id, state FROM slp_work
       WHERE team_id = ? AND generation = ? AND state <> 'DONE'
       ORDER BY created_at, id`,
    )
    .all(team.team_id, team.generation);
}

function stopResult(team: ActiveLocalTeam, emergency: boolean): CliResult {
  return {
    data: {
      emergency,
      team: {
        generation: team.generation,
        projectPath: team.project_path,
        state: "STOPPED",
        teamId: team.team_id,
      },
    },
    text: `${team.team_id} generation ${team.generation} STOPPED${emergency ? " (emergency)" : ""}`,
  };
}

interface RoomTeamRow {
  configuration_json: string;
  generation: number;
  objective: string;
  pack_digest: string;
  pack_version: string;
  project_path: string;
  state: "RUNNING" | "STOPPED";
  team_id: string;
  workspace_id: string;
}

type StopReservation =
  | { kind: "claimed"; row: SlpLifecycleRow }
  | { kind: "stopped" }
  | { kind: "wait" };

function reserveStop(
  store: Store,
  team: ActiveLocalTeam,
  roomTeam: RoomTeamRow,
  input: {
    emergency: boolean;
    ownerToken: string;
    reason: string;
  },
): StopReservation {
  return withImmediateTransaction(store, () => {
    const localState = store.database
      .query<{ state: "RUNNING" | "STOPPED" }, [string, number]>(
        "SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?",
      )
      .get(team.team_id, team.generation)?.state;
    const roomState = store.database
      .query<{ state: "RUNNING" | "STOPPED" }, [string, number]>(
        `SELECT state FROM slp_room.slp_teams
         WHERE team_id = ? AND generation = ?`,
      )
      .get(team.team_id, team.generation)?.state;
    if (localState === "STOPPED" && roomState === "STOPPED") return { kind: "stopped" };
    if (localState !== "RUNNING" || roomState !== "RUNNING") {
      throw new CliError(
        "INVALID_STATE",
        `${team.team_id}:g${team.generation} has divergent Hub and workspace state`,
        { localState, roomState },
      );
    }

    const startRepair = lifecycleRow(store, team.team_id, team.generation, "START");
    if (
      startRepair?.phase === "COMMITTED" &&
      startRepair.owner_token &&
      lifecycleOwnerIsAlive(startRepair.owner_pid)
    ) {
      return { kind: "wait" };
    }
    if (startRepair?.phase === "COMMITTED" && startRepair.owner_token) {
      const now = new Date().toISOString();
      const clear = (table: string) =>
        store.database
          .query(
            `UPDATE ${table}
             SET owner_token = NULL, owner_pid = NULL,
                 revision = revision + 1, updated_at = ?
             WHERE team_id = ? AND generation = ? AND operation = 'START'
               AND owner_token = ? AND phase = 'COMMITTED'`,
          )
          .run(now, team.team_id, team.generation, startRepair.owner_token);
      const local = clear("slp_lifecycle_operations");
      const room = clear("slp_room.slp_lifecycle_operations");
      if (local.changes !== 1 || room.changes !== 1) {
        throw new CliError(
          "SLP_LIFECYCLE_CHANGED",
          `${team.team_id}:g${team.generation} runtime repair changed before stop could proceed`,
        );
      }
    }

    const actor = "hub-supervisor";
    const reason = input.reason;
    requireRunningState(store, team);
    // w722: a normal stop never abandons work.
    if (!input.emergency) {
      const unfinished = unfinishedWork(store, team);
      if (unfinished.length > 0) {
        throw new CliError(
          "TEAM_UNFINISHED",
          `${team.team_id} has unfinished work: ${unfinished.map((work) => `${work.id} [${work.state}]`).join(", ")}`,
          { unfinished },
        );
      }
    }

    const pending = lifecycleRow(store, team.team_id, team.generation, "STOP");
    if (pending?.phase === "COMMITTED") {
      throw new CliError(
        "INVALID_STATE",
        `${team.team_id}:g${team.generation} has a committed stop but remains RUNNING`,
      );
    }
    if (pending) {
      if (
        pending.owner_token &&
        lifecycleOwnerIsAlive(pending.owner_pid)
      ) {
        return { kind: "wait" };
      }
      if (pending.emergency === 1 && !input.emergency) {
        throw new CliError(
          "TEAM_STOP_IN_PROGRESS",
          `${team.team_id}:g${team.generation} is already under emergency stop`,
        );
      }
      if (pending.emergency === 1 && pending.reason !== reason) {
        throw new CliError(
          "EMERGENCY_REASON_CHANGED",
          `${team.team_id}:g${team.generation} emergency reason is already pinned`,
          { reason: pending.reason },
        );
      }
      const effectiveActor = pending.emergency === 1 ? pending.actor : actor;
      const effectiveEmergency = pending.emergency === 1 || input.emergency;
      const effectiveReason = pending.emergency === 1 ? pending.reason : reason;
      const now = new Date().toISOString();
      const claim = (table: string) =>
        store.database
          .query(
            `UPDATE ${table}
             SET actor = ?, reason = ?, emergency = ?, owner_token = ?, owner_pid = ?,
                 revision = revision + 1, updated_at = ?
             WHERE team_id = ? AND generation = ? AND operation = 'STOP'
               AND phase <> 'COMMITTED'`,
          )
          .run(
            effectiveActor,
            effectiveReason,
            effectiveEmergency ? 1 : 0,
            input.ownerToken,
            process.pid,
            now,
            team.team_id,
            team.generation,
          );
      const local = claim("slp_lifecycle_operations");
      const room = claim("slp_room.slp_lifecycle_operations");
      if (local.changes !== 1 || room.changes !== 1) {
        throw new CliError(
          "SLP_LIFECYCLE_CHANGED",
          `${team.team_id}:g${team.generation} STOP changed before it could be claimed`,
        );
      }
      const claimed = lifecycleRow(store, team.team_id, team.generation, "STOP");
      if (!claimed) throw new Error("claimed SLP stop disappeared");
      return { kind: "claimed", row: claimed };
    }

    const now = new Date().toISOString();
    const values = [
      team.team_id,
      team.generation,
      "STOP",
      "RESERVED",
      1,
      team.project_path,
      roomTeam.objective,
      team.configuration_json,
      roomTeam.pack_version,
      team.pack_digest,
      "",
      team.workspace_id,
      actor,
      reason,
      input.emergency ? 1 : 0,
      input.ownerToken,
      process.pid,
      now,
      now,
    ] as const;
    const insert = (table: string) =>
      store.database
        .query(
          `INSERT INTO ${table}
            (team_id, generation, operation, phase, revision, project_path,
             objective, configuration_json, pack_version, pack_digest, work_id,
             workspace_id, actor, reason, emergency, owner_token, owner_pid, created_at,
             updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .run(...values);
    insert("slp_lifecycle_operations");
    insert("slp_room.slp_lifecycle_operations");
    const row = lifecycleRow(store, team.team_id, team.generation, "STOP");
    if (!row) throw new Error("reserved SLP stop disappeared");
    return { kind: "claimed", row };
  });
}

function recordStopRuntimeReady(
  store: Store,
  row: SlpLifecycleRow,
  ownerToken: string,
): SlpLifecycleRow {
  return withImmediateTransaction(store, () => {
    const now = new Date().toISOString();
    const update = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET phase = 'RUNTIME_READY', revision = revision + 1, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = 'STOP'
             AND owner_token = ? AND phase IN ('RESERVED', 'RUNTIME_READY')`,
        )
        .run(now, row.team_id, row.generation, ownerToken);
    const local = update("slp_lifecycle_operations");
    const room = update("slp_room.slp_lifecycle_operations");
    if (local.changes !== 1 || room.changes !== 1) {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${row.team_id}:g${row.generation} changed before runtime absence could be recorded`,
      );
    }
    const ready = lifecycleRow(store, row.team_id, row.generation, "STOP");
    if (!ready) throw new Error("runtime-ready SLP stop disappeared");
    return ready;
  });
}

function finalizeStop(
  store: Store,
  row: SlpLifecycleRow,
  ownerToken: string,
): SlpLifecycleRow {
  return withImmediateTransaction(store, () => {
    const current = lifecycleRow(store, row.team_id, row.generation, "STOP");
    if (!current || current.owner_token !== ownerToken || current.phase !== "RUNTIME_READY") {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${row.team_id}:g${row.generation} is not ready for stop finalization`,
      );
    }
    const now = new Date().toISOString();
    if (current.emergency === 1) {
      store.database
        .query(
          `UPDATE slp_work
           SET abandoned_at = ?, abandoned_by = ?, abandonment_reason = ?
           WHERE team_id = ? AND generation = ? AND state <> 'DONE'
             AND abandoned_at IS NULL`,
        )
        .run(
          now,
          current.actor,
          current.reason,
          current.team_id,
          current.generation,
        );
    }
    const localTransition = store.database
      .query(
        `UPDATE slp_local_teams SET state = 'STOPPED'
         WHERE team_id = ? AND generation = ? AND state = 'RUNNING'`,
      )
      .run(current.team_id, current.generation);
    const roomTransition = store.database
      .query(
        `UPDATE slp_room.slp_teams SET state = 'STOPPED', stopped_at = ?
         WHERE team_id = ? AND generation = ? AND state = 'RUNNING'`,
      )
      .run(now, current.team_id, current.generation);
    if (localTransition.changes !== 1 || roomTransition.changes !== 1) {
      throw new CliError(
        "INVALID_STATE",
        `${current.team_id}:g${current.generation} changed before team stop could commit`,
      );
    }
    const activity = current.emergency === 1 ? "team.stop.emergency" : "team.stop";
    const recordActivity = (table: string) =>
      store.database
        .query(
          `INSERT INTO ${table}
            (team_id, generation, actor, operation, target_type, target_id, created_at)
           VALUES (?, ?, ?, ?, 'team', ?, ?)`,
        )
        .run(
          current.team_id,
          current.generation,
          current.actor,
          activity,
          current.team_id,
          now,
        );
    recordActivity("slp_activity");
    recordActivity("slp_room.slp_activity");
    const commitLifecycle = (table: string) =>
      store.database
        .query(
          `UPDATE ${table}
           SET phase = 'COMMITTED', revision = revision + 1,
               owner_token = NULL, owner_pid = NULL, updated_at = ?
           WHERE team_id = ? AND generation = ? AND operation = 'STOP'
             AND owner_token = ? AND phase = 'RUNTIME_READY'`,
        )
        .run(now, current.team_id, current.generation, ownerToken);
    const local = commitLifecycle("slp_lifecycle_operations");
    const room = commitLifecycle("slp_room.slp_lifecycle_operations");
    if (local.changes !== 1 || room.changes !== 1) {
      throw new CliError(
        "SLP_LIFECYCLE_CHANGED",
        `${current.team_id}:g${current.generation} changed before stop could commit`,
      );
    }
    const committed = lifecycleRow(store, current.team_id, current.generation, "STOP");
    if (!committed) throw new Error("committed SLP stop disappeared");
    return committed;
  });
}

async function stopTeam(
  context: PluginContext,
  runtime: HerdrSlpRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const requestedTeam = requiredPosition(invocation, 0, "team id");
  const emergency = invocation.options.emergency === true;
  const requestedReason = stringOption(invocation, "reason");
  // Under --emergency the reason says why unfinished work is abandoned; on a
  // normal stop it is the closing report (stopSuffix reads it back as
  // "(supervisor): ...").
  const stopReason = emergency
    ? requestedReason ?? "Hub Supervisor emergency stop"
    : requestedReason ?? "";
  if (!isRoom(context.store.database)) {
    throw new CliError("ROLE_FORBIDDEN", "team stop is Hub Supervisor authority and must run from ~/maestro");
  }
  migrateRoom(context.store);
  if (!tableExists(context.store, "slp_teams")) {
    throw new CliError("NOT_FOUND", `SLP team not found: ${requestedTeam}`);
  }
  const roomTeam = context.store.database
    .query<RoomTeamRow, [string]>(
      `SELECT team_id, generation, project_path, configuration_json,
              objective, pack_version, pack_digest, state, workspace_id
       FROM slp_teams WHERE team_id = ?
       ORDER BY generation DESC LIMIT 1`,
    )
    .get(requestedTeam);
  if (!roomTeam) throw new CliError("NOT_FOUND", `SLP team not found: ${requestedTeam}`);

  const projectStore = new Store(resolveStoreLocation(roomTeam.project_path).path);
  migrateProject(projectStore);
  const team = projectStore.database
    .query<ActiveLocalTeam, [string, number]>(
      `SELECT team_id, generation, room_store_path, project_path,
              configuration_json, pack_digest, workspace_id, runtime_pane_id,
              supervisor_pane_id
       FROM slp_local_teams WHERE team_id = ? AND generation = ?`,
    )
    .get(roomTeam.team_id, roomTeam.generation);
  if (!team) {
    projectStore.close();
    throw new CliError(
      "SLP_BINDING_MISSING",
      `project binding missing for ${roomTeam.team_id}:g${roomTeam.generation}`,
    );
  }
  if (resolve(team.room_store_path) !== resolve(context.store.path)) {
    projectStore.close();
    throw new CliError("INVALID_STOP_GRANT", `${team.team_id} is bound to another Hub`);
  }

  let attachedRoom = false;
  try {
    if (roomTeam.state === "STOPPED") {
      await runtime.stop(await stopPlan(team), stopRoles(projectStore, team));
      context.sessions.record(emergency ? "team.stop.emergency" : "team.stop");
      return {
        data: {
          emergency,
          team: {
            generation: team.generation,
            projectPath: team.project_path,
            state: "STOPPED",
            teamId: team.team_id,
          },
        },
        text: `${team.team_id} generation ${team.generation} already STOPPED; runtime cleanup complete`,
      };
    }
    projectStore.database.query("ATTACH DATABASE ? AS slp_room").run(team.room_store_path);
    attachedRoom = true;
    projectStore.database.exec("PRAGMA busy_timeout = 300000");
    const ownerToken = randomUUID();
    const deadline = Date.now() + 30_000;
    let reservation: StopReservation;
    while (true) {
      reservation = reserveStop(projectStore, team, roomTeam, {
        emergency,
        ownerToken,
        reason: stopReason,
      });
      if (reservation.kind !== "wait") break;
      if (Date.now() >= deadline) {
        throw new CliError(
          "TEAM_STOP_PENDING",
          `${team.team_id}:g${team.generation} is still being stopped by another process`,
        );
      }
      await Bun.sleep(50);
    }
    if (reservation.kind === "stopped") {
      await runtime.stop(await stopPlan(team), stopRoles(projectStore, team));
      return stopResult(team, emergency);
    }
    let operation = reservation.row;
    try {
      const roles = stopRoles(projectStore, team);
      await runtime.stop(await stopPlan(team), roles);
      operation = recordStopRuntimeReady(projectStore, operation, ownerToken);
      operation = finalizeStop(projectStore, operation, ownerToken);
    } catch (error) {
      try {
        releaseLifecycleOwner(projectStore, operation, ownerToken);
      } catch {}
      throw error;
    }
    const committedEmergency = operation.emergency === 1;
    context.sessions.record(committedEmergency ? "team.stop.emergency" : "team.stop");
    return stopResult(team, committedEmergency);
  } finally {
    if (attachedRoom) projectStore.database.exec("DETACH DATABASE slp_room");
    projectStore.close();
  }
}

function runtimePaneIdInProject(projectPath: string, teamId: string, generation: number): string {
  const location = resolveStoreLocation(projectPath);
  if (!existsSync(location.path)) return "";
  const store = new Store(location.path, { readonly: true });
  try {
    if (!tableExists(store, "slp_local_teams") || !store.hasColumn("slp_local_teams", "runtime_pane_id")) return "";
    return runtimePaneIdOf(store, teamId, generation);
  } finally {
    store.close();
  }
}

function roleRows(store: Store, teamId: string, generation: number, local: boolean) {
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

type SlpStatusRole = ReturnType<typeof roleRows>[number];

interface SlpNextStep {
  mayRun: string[];
  waitingOn: string | null;
}

// l9 (w739, d896): the text read printed the first line of a 4,000-word brief
// and said nothing about the rest, so a seat read its item, believed it had read
// the request, and was wrong. A clipped objective now names its own size and the
// command that prints it whole. An objective that fits on one line is unchanged.
function objectiveLines(id: string, objective: string, limit: number): string[] {
  const head = clipLine(objective, limit);
  const full = objective.trim();
  if (full === head) return [`objective: ${head}`];
  const lines = full.split("\n").length;
  return [
    `objective: ${head}`,
    `objective truncated here: ${full.length} characters over ${lines} line${lines === 1 ? "" : "s"};` +
      ` read it in full with: maestro status ${id} --json`,
  ];
}

function clipLine(text: string, limit: number): string {
  const line = text.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > limit ? `${line.slice(0, limit - 3)}...` : line;
}

// room d129 point 4: the capture points are recorded, never inferred (d705).
// A reviewer's rework grant and a decision that both --replaces and names
// --work are the two moments a correction is already being written down; the
// row is the mark, and no text is read to produce it.
function markLessonCandidate(store: Store, workId: string, source: string, at: string): void {
  if (!tableExists(store, "slp_lesson_candidates")) return;
  store.database
    .query(
      `INSERT OR IGNORE INTO slp_lesson_candidates (work_id, source, recorded_at)
       VALUES (?, ?, ?)`,
    )
    .run(workId, source, at);
}

function lessonCandidateSources(store: Store, workId: string): string[] {
  if (!tableExists(store, "slp_lesson_candidates")) return [];
  return store.database
    .query<{ source: string }, [string]>(
      "SELECT source FROM slp_lesson_candidates WHERE work_id = ? ORDER BY recorded_at, source",
    )
    .all(workId)
    .map((row) => row.source);
}

function reworkGrantOpen(context: PluginContext, work: SlpWorkRow): boolean {
  if (work.state !== "RETURNED" || !tableExists(context.store, "slp_rework_grants")) return false;
  return context.store.database
    .query<{ present: number }, [string, number]>(
      `SELECT 1 AS present FROM slp_rework_grants
       WHERE work_id = ? AND return_revision = ? AND consumed_at IS NULL`,
    )
    .get(work.id, work.return_revision) !== null;
}

// d758: what the caller may run on one item, or whom it waits on.
function nextStep(
  actor: SlpReviewerActor,
  roles: SlpStatusRole[],
  work: SlpWorkRow,
  grantOpen: boolean,
): SlpNextStep {
  const assigneeRole = roles.find((role) => role.name === work.assigned_to)?.role ?? "peer";
  // room d117: the Lead's reviewer is the Hub, which holds no seat here - so
  // no seat is ever "reviewing" such an item, the waiting-on line names the
  // Hub, and the Hub itself reads the accept and rework lines when it is the
  // one asking.
  const hubReviews = assigneeRole === "lead";
  const reviewerName = hubReviews
    ? hubReviewerName
    : roles.find((role) => role.role === "lead")?.name ?? "lead";
  const reviewing = hubReviews
    ? actor.role === hubReviewerName
    : actor.role === "lead" && actor.name !== work.assigned_to;
  const mine = work.assigned_to === actor.name;
  switch (work.state) {
    case "OPEN":
      return mine
        ? { mayRun: [`work take ${work.id}`], waitingOn: null }
        : {
          mayRun: reviewing ? [`work accept ${work.id} --outcome cancelled`] : [],
          waitingOn: work.assigned_to,
        };
    case "ACTIVE":
      return work.owner === actor.name
        ? { mayRun: [`work return ${work.id} "<result>"`], waitingOn: null }
        : { mayRun: [], waitingOn: work.owner ?? work.assigned_to };
    case "RETURNED":
      if (grantOpen) {
        return mine
          ? { mayRun: [`work take ${work.id}`], waitingOn: null }
          : { mayRun: reviewing ? [`work accept ${work.id}`] : [], waitingOn: work.assigned_to };
      }
      return reviewing
        ? {
          mayRun: [
            `work accept ${work.id}`,
            `work note ${work.id} "<gap>" --rework`,
            `work accept ${work.id} --outcome cancelled`,
          ],
          waitingOn: null,
        }
        : { mayRun: [], waitingOn: reviewerName };
    default:
      return { mayRun: [], waitingOn: null };
  }
}

function nextLine(work: SlpWorkRow, step: SlpNextStep, lessonCandidate = false): string {
  if (work.state === "DONE") return `next: none (${work.acceptance_outcome ?? "done"})`;
  const optional = step.mayRun.length > 0 ? `; may run: ${step.mayRun.join(" | ")}` : "";
  const base = step.waitingOn === null
    ? `next: ${step.mayRun.join(" | ")}`
    : `next: waiting on ${step.waitingOn}${optional}`;
  if (!lessonCandidate) return base;
  // room d129 point 4: the reminder is addressed to whoever the next step is
  // addressed to, so the holder reads what its return must say and the
  // reviewer reads what it may accept on.
  const may = (verb: string) => step.mayRun.some((command) => command.startsWith(`${verb} ${work.id}`));
  const reminder = may("work accept")
    ? "accept only after the return states the lesson it filed or why none applies"
    : may("work return") || may("work take")
    ? "this item carries a lesson candidate: state in the return the lesson you filed (maestro lesson file) or why none applies"
    : "this item carries a lesson candidate";
  return `${base}; ${reminder}`;
}

// w707: DONE is the one state that means two opposite things - delivered, or
// cancelled without ever being worked - and the store already knows which, in
// acceptance_outcome. Print it on every DONE line and in the collapsed count
// rather than marking only the exception: an unmarked line would leave the
// reader to supply "accepted", which is exactly the join w706 stopped asking
// readers to make. No state is added; DONE is still DONE.
function doneOutcome(work: SlpWorkRow): string {
  return work.acceptance_outcome ?? "outcome unrecorded";
}

function doneSummary(done: SlpWorkRow[]): string {
  const tally = new Map<string, number>();
  for (const work of done) {
    const outcome = doneOutcome(work);
    tally.set(outcome, (tally.get(outcome) ?? 0) + 1);
  }
  const known = ["accepted", "cancelled"];
  const rank = (outcome: string) => {
    const at = known.indexOf(outcome);
    return at < 0 ? known.length : at;
  };
  const outcomes = [...tally.keys()].sort((left, right) =>
    rank(left) - rank(right) || left.localeCompare(right)
  );
  const first = outcomes[0];
  const summary = outcomes.length === 1 && first !== undefined
    ? first
    : outcomes.map((outcome) => `${tally.get(outcome)} ${outcome}`).join(", ");
  return `${done.length} DONE (${summary}); --all to list`;
}

function workLine(work: SlpWorkRow, step: SlpNextStep): string {
  const marker = work.state !== "DONE" && step.waitingOn === null ? "*" : " ";
  const outcome = work.state === "DONE" ? ` (${doneOutcome(work)})` : "";
  return `${marker} ${work.id} ${work.state}${outcome} ${work.created_by} -> ${work.assigned_to}: ${
    clipLine(work.objective, 72)
  }`;
}

function decisionLine(refs: Array<{ id: string; workId: string | null }>): string {
  if (refs.length === 0) return "decisions: none";
  return `decisions: ${
    refs.map((ref) => ref.workId ? `${ref.id} (${ref.workId})` : ref.id).join(", ")
  }`;
}

function teamDecisionRefs(
  context: PluginContext,
  actor: SlpActor,
): Array<{ id: string; workId: string | null }> {
  const query = (store: Store) =>
    tableExists(store, "slp_decisions")
      ? store.database
        .query<{ created_at: string; id: string; work_id: string | null }, [string, number]>(
          `SELECT id, work_id, created_at FROM slp_decisions
           WHERE team_id = ? AND generation = ? ORDER BY created_at, id`,
        )
        .all(actor.team.team_id, actor.team.generation)
      : [];
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let rows = query(context.store);
  try {
    rows = [...rows, ...query(roomStore)];
  } finally {
    roomStore.close();
  }
  return [...new Map(rows.map((row) => [row.id, row])).values()]
    .sort((left, right) =>
      left.created_at.localeCompare(right.created_at) || left.id.localeCompare(right.id)
    )
    .map((row) => ({ id: row.id, workId: row.work_id }));
}

const decisionIdPattern = /^d\d+$/;

interface SlpDecisionRow {
  actor: string;
  choice: string;
  created_at: string;
  generation: number;
  id: string;
  provisional: number;
  replaces_id: string | null;
  scope: string;
  team_id: string;
  why: string;
  work_id: string | null;
}

interface SlpDecisionData {
  actor: string;
  choice: string;
  createdAt: string;
  id: string;
  provisional: boolean;
  replaces: string | null;
  scope: string;
  store: "hub" | "project";
  why: string;
  workId: string | null;
}

// A seat reads its own store's rows unfiltered; from the Hub store it reads
// its own team's rows plus the owner and cross-team rulings, which carry no
// work id and belong to no team (teamId null asks for the whole store).
function decisionRowsById(store: Store, id: string, teamId: string | null): SlpDecisionRow[] {
  if (!tableExists(store, "slp_decisions")) return [];
  const columns =
    `SELECT id, team_id, generation, choice, why, scope, work_id, replaces_id, actor, created_at,
            ${provisionalColumn(store)}
     FROM slp_decisions WHERE id = ?`;
  return teamId === null
    ? store.database.query<SlpDecisionRow, [string]>(columns).all(id)
    : store.database
      .query<SlpDecisionRow, [string, string]>(
        `${columns} AND (team_id = ? OR scope IN ('owner', 'cross-team'))`,
      )
      .all(id, teamId);
}

function decisionData(row: SlpDecisionRow, store: "hub" | "project"): SlpDecisionData {
  return {
    actor: row.actor,
    choice: row.choice,
    createdAt: row.created_at,
    id: row.id,
    provisional: row.provisional === 1,
    replaces: row.replaces_id,
    scope: row.scope,
    store,
    why: row.why,
    workId: row.work_id,
  };
}

// w736 F4: a store can hold two decision spaces at once - the project decision
// ledger (`decisions`, with the draft/lock/withdraw lifecycle) and the SLP
// rulings (`slp_decisions`) - and their ids are separate sequences that already
// overlap. `maestro status dNNN` reads the second; `maestro decision show dNNN`
// read only the first and answered NOT_FOUND for every ruling that had been
// recorded. It now reads both and labels each, which is w697's shape: a store
// never silently wins an id it shares.
//
// l4 (w719, w720): F4 asked this of the Hub room only, so inside a project
// checkout every SLP decision a team records stayed NOT_FOUND to the same verb
// that prints it one directory up - and a reader who gets NOT_FOUND from one
// store and a row from another learns the opposite of where the row lives. The
// space is read from whichever store the command runs in, and the label names
// that store rather than the seat that authored the row, which is the label
// l4 found pointing the wrong way.
function slpDecisionResult(
  context: PluginContext,
  id: string,
): { decisions: unknown[]; text: string } | null {
  const store = isRoom(context.store.database) ? "hub" : "project";
  const decisions = decisionRowsById(context.store, id, null).map((row) => decisionData(row, store));
  if (decisions.length === 0) return null;
  return { decisions, text: decisions.map(decisionText).join("\n\n") };
}

function decisionText(decision: SlpDecisionData): string {
  return [
    `${decision.id} [${decision.scope}] by ${decision.actor} (${decision.store} store)`,
    `work: ${decision.workId ?? "none"}`,
    ...(decision.provisional ? ["provisional: yes"] : []),
    ...(decision.replaces ? [`replaces: ${decision.replaces}`] : []),
    `choice: ${decision.choice}`,
    `why: ${decision.why}`,
  ].join("\n");
}

// w697: decision ids are per-store sequences, so the same id can in principle
// name a row in both stores. Every match is returned carrying the store it came
// from rather than one store silently winning; the two stores hold no
// overlapping id today (d857), so the plural case is a shape, not a claim.
function decisionResult(decisions: readonly SlpDecisionData[], id: string): CliResult {
  if (decisions.length === 0) throw new CliError("NOT_FOUND", `SLP decision not found: ${id}`);
  return {
    data: { decisions },
    text: decisions.map(decisionText).join("\n\n"),
  };
}

function readSlpDecision(context: PluginContext, actor: SlpActor, id: string): CliResult {
  const local = decisionRowsById(context.store, id, null)
    .map((row) => decisionData(row, "project"));
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let hub: SlpDecisionData[] = [];
  try {
    hub = decisionRowsById(roomStore, id, actor.team.team_id)
      .map((row) => decisionData(row, "hub"));
  } finally {
    roomStore.close();
  }
  return decisionResult([...local, ...hub], id);
}

// room d117: one work-detail read, shared by the seats and by the Hub Supervisor
// reviewing the Lead's work. Extracted verbatim from maybeHandleSlpStatus so
// the Hub sees exactly what a seat sees - same entries, same merged
// decisions, same next: line - rather than a second, drifting renderer.
async function readWorkDetail(
  context: PluginContext,
  actor: SlpReviewerActor,
  roles: SlpStatusRole[],
  workId: string,
): Promise<CliResult> {
  const work = requireSlpWork(context, actor, workId);
  if (actor.role === "peer" && work.assigned_to !== actor.name) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${workId}`);
  }
  if (
    actor.role === "lead" &&
    work.assigned_to !== actor.name &&
    work.created_by !== actor.name
  ) {
    throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${workId}`);
  }
  const entries = context.store.database
    .query<{
      actor: string;
      body: string;
      created_at: string;
      flag: string | null;
      kind: "NOTE" | "RETURN" | "ACCEPTANCE";
    }, [string]>(
      `SELECT kind, actor, body, flag, created_at FROM slp_work_entries
       WHERE work_id = ? ORDER BY id`,
    )
    .all(workId);
  const localDecisions = context.store.database
    .query<{
      actor: string;
      choice: string;
      created_at: string;
      id: string;
      replaces_id: string | null;
      scope: string;
      why: string;
    }, [string]>(
      `SELECT id, choice, why, scope, replaces_id, actor, created_at
       FROM slp_decisions WHERE work_id = ? ORDER BY created_at, id`,
    )
    .all(workId)
    .map((decision) => ({
      actor: decision.actor,
      choice: decision.choice,
      createdAt: decision.created_at,
      id: decision.id,
      replaces: decision.replaces_id,
      scope: decision.scope,
      why: decision.why,
    }));
  const roomStore = new Store(actor.team.room_store_path, { readonly: true });
  let hubDecisions: typeof localDecisions = [];
  try {
    if (tableExists(roomStore, "slp_decisions")) {
      hubDecisions = roomStore.database
        .query<{
          actor: string;
          choice: string;
          created_at: string;
          id: string;
          replaces_id: string | null;
          scope: string;
          why: string;
        }, [string, number, string]>(
          `SELECT id, choice, why, scope, replaces_id, actor, created_at
           FROM slp_decisions
           WHERE team_id = ? AND generation = ? AND work_id = ?
           ORDER BY created_at, id`,
        )
        .all(actor.team.team_id, actor.team.generation, workId)
        .map((decision) => ({
          actor: decision.actor,
          choice: decision.choice,
          createdAt: decision.created_at,
          id: decision.id,
          replaces: decision.replaces_id,
          scope: decision.scope,
          why: decision.why,
        }));
    }
  } finally {
    roomStore.close();
  }
  const decisions = [...new Map(
    [...localDecisions, ...hubDecisions].map((decision) => [decision.id, decision]),
  ).values()].sort((left, right) =>
    left.createdAt.localeCompare(right.createdAt) || left.id.localeCompare(right.id)
  );
  const latest = entries.at(-1) ?? null;
  const candidateSources = lessonCandidateSources(context.store, workId);
  // room d137: the assignee's compact count, read beside its seat and nowhere
  // else. It is evidence for the reviewer's judgment, never a gate: no branch
  // below it refuses, nudges or stops anything on the figure.
  const compacts = compactLine(
    compactsForPane(
      context.store,
      roles.find((role) => role.name === work.assigned_to)?.pane_id ?? null,
    ),
  );
  return {
    data: {
      acceptance: entries.findLast((entry) => entry.kind === "ACCEPTANCE") ?? null,
      decisions,
      lessonCandidate: candidateSources.length > 0 ? { sources: candidateSources } : null,
      notes: entries
        .filter((entry) => entry.kind === "NOTE")
        .map((entry) => ({
          actor: entry.actor,
          body: entry.body,
          createdAt: entry.created_at,
          flag: entry.flag,
        })),
      returns: entries
        .filter((entry) => entry.kind === "RETURN")
        .map((entry) => ({ actor: entry.actor, body: entry.body, createdAt: entry.created_at })),
      work: workData(work),
    },
    text: [
      `${work.id} ${work.state} ${work.created_by} -> ${work.assigned_to}${compacts ? ` ${compacts}` : ""}`,
      `revision: ${work.return_revision}`,
      ...objectiveLines(work.id, work.objective, 160),
      latest
        ? `${latest.kind.toLowerCase()}${latest.flag ? ` [${latest.flag}]` : ""} by ${latest.actor}: ${
          clipLine(latest.body, 160)
        }`
        : "entries: none",
      decisionLine(decisions.map((decision) => ({ id: decision.id, workId: null }))),
      ...(candidateSources.length > 0
        ? [`lesson candidate: yes, from ${candidateSources.join(", ")}`]
        : []),
      nextLine(
        work,
        nextStep(actor, roles, work, reworkGrantOpen(context, work)),
        candidateSources.length > 0,
      ),
    ].join("\n"),
  };
}

async function maybeHandleSlpStatus(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  const requestedWork = invocation.positionals[0] ?? null;
  if (isRoom(context.store.database)) {
    if (requestedWork) {
      // w697: the Hub reads its own rulings by id; work still belongs to the seats.
      if (decisionIdPattern.test(requestedWork)) {
        return decisionResult(
          decisionRowsById(context.store, requestedWork, null)
            .map((row) => decisionData(row, "hub")),
          requestedWork,
        );
      }
      // room d117: the Hub reads the work it reviews.
      const bound = bindHubReviewer(context, requestedWork, "read");
      try {
        const roles = roleRows(
          bound.context.store,
          bound.actor.team.team_id,
          bound.actor.team.generation,
          true,
        );
        return await readWorkDetail(bound.context, bound.actor, roles, bound.workId);
      } finally {
        bound.close();
      }
    }
    const rows = tableExists(context.store, "slp_teams")
      ? context.store.database
      .query<{
        configuration_json: string;
        generation: number;
        pack_digest: string;
        pack_version: string;
        project_path: string;
        state: "RUNNING" | "STOPPED";
        team_id: string;
        workspace_id: string;
      }, []>(
        `SELECT team_id, generation, project_path, configuration_json, pack_version,
                pack_digest, state, workspace_id
         FROM slp_teams
         ORDER BY project_path, generation`,
      )
      .all()
      : [];
    const runtime = new HerdrSlpRuntime();
    const teams = [] as Array<Record<string, unknown>>;
    for (const row of rows) {
      const roles = roleRows(context.store, row.team_id, row.generation, false);
      let missingPanes: string[] = [];
      let runtimeState: "available" | "unavailable" | "not-running" = "not-running";
      let runtimePane = false;
      const plan = await planForTeam(row);
      const runtimePaneId = runtimePaneIdInProject(row.project_path, row.team_id, row.generation);
      try {
        const inspection = await runtime.inspect(
          plan,
          roles.map((role) => ({
            briefDigest: role.brief_digest,
            instanceId: role.instance_id,
            name: role.name,
            packDigest: role.pack_digest,
            paneId: role.pane_id,
            profile: role.profile,
            readyChallenge: role.ready_challenge,
            role: role.role,
            workspaceId: role.workspace_id,
          })),
          runtimePaneId,
        );
        if (row.state === "RUNNING" || inspection.workspace) {
          missingPanes = inspection.missingPanes;
          runtimeState = inspection.runtime;
          runtimePane = inspection.runtimePane;
        }
      } catch {
        runtimeState = "unavailable";
      }
      const counts: Record<WorkState, number> = { ACTIVE: 0, DONE: 0, OPEN: 0, RETURNED: 0 };
      let abandonedWorkCount = 0;
      let hubBlocked: Array<{ body: string; workId: string }> = [];
      const projectStore = new Store(resolveStoreLocation(row.project_path).path, { readonly: true });
      try {
        if (row.state === "RUNNING") hubBlocked = blockedForHub(context.store, projectStore, row, roles);
        if (tableExists(projectStore, "slp_work")) {
          for (const count of projectStore.database
            .query<{ count: number; state: WorkState }, [string, number]>(
              `SELECT state, COUNT(*) AS count FROM slp_work
               WHERE team_id = ? AND generation = ? GROUP BY state`,
            )
            .all(row.team_id, row.generation)) {
            counts[count.state] = count.count;
          }
          const hasAbandonment = projectStore.database
            .query<{ present: number }, []>(
              `SELECT 1 AS present FROM pragma_table_info('slp_work')
               WHERE name = 'abandoned_at'`,
            )
            .get();
          if (hasAbandonment) {
            abandonedWorkCount = projectStore.database
              .query<{ count: number }, [string, number]>(
                `SELECT COUNT(*) AS count FROM slp_work
                 WHERE team_id = ? AND generation = ? AND abandoned_at IS NOT NULL`,
              )
              .get(row.team_id, row.generation)?.count ?? 0;
          }
        }
      } finally {
        projectStore.close();
      }
      // d870's third clause (d873): a REPORT-ONLY orphaned verdict. A RUNNING
      // generation whose Lead is missing has no seat left to report on it, so
      // the Hub is told, and told the one command that closes it by hand. It
      // is deliberately not certainty: this branch reads through
      // HerdrSlpRuntime.inspect, whose zero-workspace path reports EVERY role
      // missing, so a Herdr that is down or has not restored its workspace is
      // indistinguishable from a real orphan here. That is exactly why it is
      // wired to no commit. A status read mutates nothing. The non-empty
      // guard keeps a generation with no rows at all from reading as orphaned
      // rather than as unknown.
      const leads = roles.filter((role) => role.role === "lead");
      const orphaned = row.state === "RUNNING" &&
        leads.length > 0 &&
        leads.every((role) => missingPanes.includes(role.name));
      const stopRecord = tableExists(context.store, "slp_lifecycle_operations")
        ? context.store.database
          .query<{ actor: string; emergency: number; reason: string }, [string, number]>(
            `SELECT actor, emergency, reason FROM slp_lifecycle_operations
             WHERE team_id = ? AND generation = ? AND operation = 'STOP' AND phase = 'COMMITTED'`,
          )
          .get(row.team_id, row.generation) ?? null
        : null;
      teams.push({
        abandonedWorkCount,
        generation: row.generation,
        hubBlocked,
        missingPanes,
        orphaned,
        packDigest: row.pack_digest,
        packVersion: row.pack_version,
        projectPath: row.project_path,
        roles: roles.map((role) => ({
          briefDigest: role.brief_digest,
          instanceId: role.instance_id,
          name: role.name,
          packDigest: role.pack_digest,
          paneId: role.pane_id,
          profile: role.profile,
          readyChallenge: role.ready_challenge,
          role: role.role,
        })),
        runtime: runtimeState,
        state: row.state,
        stop: stopRecord
          ? { actor: stopRecord.actor, emergency: stopRecord.emergency === 1, reason: stopRecord.reason }
          : null,
        runtimePane: runtimePane ? "on" : "off",
        teamId: row.team_id,
        workCounts: counts,
        workspaceId: row.workspace_id,
      });
    }
    const waiting = provisionalDecisions(context.store);
    return {
      data: { provisionalDecisions: waiting, teams },
      text: [
        ...(teams.length === 0
          ? ["no SLP teams"]
          : teams.flatMap((team) => [
            `${team.teamId} g${team.generation} ${team.state}${stopSuffix(team.stop as { emergency: boolean; reason: string } | null)}; runtime pane ${team.runtimePane}; missing ${(team.missingPanes as string[]).join(", ") || "none"}${orphanedSuffix(team)}`,
            ...(team.hubBlocked as Array<{ body: string; workId: string }>).map((blocked) =>
              `  blocked for the Hub: ${team.teamId}:${blocked.workId} ${clipLine(blocked.body, 160)}; answer: maestro decide "<choice>" --why "<reason>" --work ${team.teamId}:${blocked.workId}`
            ),
          ])),
        ...provisionalLines(waiting),
      ].join("\n"),
    };
  }

  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["lead", "peer"]);
  const roles = roleRows(context.store, actor.team.team_id, actor.team.generation, true);
  if (requestedWork && decisionIdPattern.test(requestedWork)) {
    return readSlpDecision(context, actor, requestedWork);
  }
  if (requestedWork) {
    return readWorkDetail(context, actor, roles, requestedWork);
  }

  const allWork = context.store.database
    .query<SlpWorkRow, [string, number]>(
      `SELECT * FROM slp_work WHERE team_id = ? AND generation = ? ORDER BY created_at, id`,
    )
    .all(actor.team.team_id, actor.team.generation);
  const scoped = allWork.filter((work) => {
    if (actor.role === "peer") return work.assigned_to === actor.name && work.state !== "DONE";
    return (
      work.assigned_to === actor.name || work.created_by === actor.name
    ) && work.state !== "DONE";
  });
  let missingPanes: string[] = [];
  let runtimeState: "available" | "unavailable" = "available";
  let runtimePane = false;
  const plan = await planForTeam(actor.team);
  try {
    const inspection = await new HerdrSlpRuntime().inspect(
      plan,
      roles.map((role) => ({
        briefDigest: role.brief_digest,
        instanceId: role.instance_id,
        name: role.name,
        packDigest: role.pack_digest,
        paneId: role.pane_id,
        profile: role.profile,
        readyChallenge: role.ready_challenge,
        role: role.role,
        workspaceId: role.workspace_id,
      })),
      actor.team.runtime_pane_id,
    );
    missingPanes = inspection.missingPanes;
    runtimePane = inspection.runtimePane;
  } catch {
    runtimeState = "unavailable";
  }
  return {
    data: {
      generation: actor.team.generation,
      missingPanes,
      role: { name: actor.name, role: actor.role },
      roles: roles.map((role) => ({
        briefDigest: role.brief_digest,
        instanceId: role.instance_id,
        name: role.name,
        packDigest: role.pack_digest,
        paneId: role.pane_id,
        profile: role.profile,
        readyChallenge: role.ready_challenge,
        role: role.role,
      })),
      runtime: runtimeState,
      runtimePane: runtimePane ? "on" : "off",
      teamId: actor.team.team_id,
      work: scoped.map(workData),
    },
    text: teamStatusText(context, invocation, actor, roles, allWork, missingPanes),
  };
}

// d758: text lists the team's items; JSON keeps the relevance-scoped list.
function teamStatusText(
  context: PluginContext,
  invocation: CliInvocation,
  actor: SlpActor,
  roles: SlpStatusRole[],
  allWork: SlpWorkRow[],
  missingPanes: string[],
): string {
  const order: Record<WorkState, number> = { ACTIVE: 1, DONE: 3, OPEN: 0, RETURNED: 2 };
  const visible = actor.role === "peer"
    ? allWork.filter((work) => work.assigned_to === actor.name)
    : allWork;
  const pending = visible
    .filter((work) => work.state !== "DONE")
    .sort((left, right) => order[left.state] - order[right.state]);
  const done = visible.filter((work) => work.state === "DONE");
  const line = (work: SlpWorkRow) =>
    workLine(work, nextStep(actor, roles, work, reworkGrantOpen(context, work)));
  const pane = roles.find((role) => role.name === actor.name)?.pane_id ?? "";
  const missing = missingPanes.length > 0 ? `; missing ${missingPanes.join(", ")}` : "";
  return [
    `${actor.team.team_id} g${actor.team.generation} ${actor.role} ${actor.name} in ${pane}${missing}`,
    ...pending.map(line),
    ...(invocation.options.all === true
      ? done.map(line)
      : done.length > 0
      ? [doneSummary(done)]
      : []),
    decisionLine(teamDecisionRefs(context, actor)),
  ].join("\n");
}

const workAddFlags = {
  "--to": {
    description: "Assign SLP work to a Peer; without it the Lead opens an item for itself.",
    value: true,
  },
  "--model": {
    description: "Lead only: launch the --to Peer with this model on its launch line (default: the generation's --peer-model, else the peer seat file's model); passed through unvalidated (room d128), so name a model the peer seat's harness accepts, or its pane dies on the harness's own error.",
    value: true,
  },
  "--fresh": {
    description: "Lead only: reset a reused --to Peer pane to a fresh harness context (Claude /clear, Codex /new), proven by a new harness session, and re-check READY before the OPEN push; a no-op on a pane being opened, refused while the Peer holds ACTIVE work or when the reset does not take.",
  },
};

const workNoteFlags = {
  "--blocked": {
    description: "Flag the note as blocked and push it one SLP seat up.",
  },
  "--owner": {
    description: "Record an owner walk-in typed into this seat's pane (room d123): provenance only, pushed one SLP seat up, never a blocked flag.",
  },
  "--rework": {
    description: "Grant the current SLP return revision one reviewer-authorized retake.",
  },
};

function registerCoreSeams(context: PluginContext): void {
  context.effect(() => context.cli.guard(paneGuard(context.store)));
  for (const [flag, definition] of Object.entries(workAddFlags)) {
    context.effect(() => context.cli.registerFlag("work add", flag, definition));
  }
  for (const [flag, definition] of Object.entries(workNoteFlags)) {
    context.effect(() => context.cli.registerFlag("work note", flag, definition));
  }
  context.effect(() =>
    context.cli.registerFlag("install", "--seat-token", {
      description: "Read one claude setup-token token from stdin and store it for every Claude seat (d846).",
    })
  );
  context.effect(() =>
    context.events.on<CliInvocation, CliResult>("work.add.invoke", async (invocation, next) => {
      if (invocation.options.start === true && invocation.options.to !== undefined) {
        throw new CliError("INVALID_OPTION", "--start is not available for SLP work add --to <peer>; the Peer takes its card with work take");
      }
      const slp = await maybeHandleSlpWorkAdd(context, invocation);
      if (slp) return slp;
      // Hub finding F1 (g7, w734): an option only a Lead's Peer add can honour refuses before any store write.
      if (invocation.options.model !== undefined) {
        throw new CliError("INVALID_OPTION", "--model is available only for SLP work add --to <peer>");
      }
      if (invocation.options.fresh !== undefined) {
        throw new CliError("INVALID_OPTION", "--fresh is available only for SLP work add --to <peer>");
      }
      return next();
    })
  );
  context.effect(() =>
    context.events.on<CliInvocation, CliResult>("work.note.invoke", async (invocation, next) => {
      const slp = await maybeHandleSlpWorkNote(context, invocation);
      if (slp) return slp;
      if (invocation.options.rework === true) {
        throw new CliError("INVALID_OPTION", "--rework is available only for SLP RETURNED work");
      }
      if (invocation.options.blocked === true) {
        throw new CliError("INVALID_OPTION", "--blocked is available only for SLP work");
      }
      if (invocation.options.owner === true) {
        throw new CliError("INVALID_OPTION", "--owner is available only for SLP work held or reviewed by a seat");
      }
      return next();
    })
  );
  context.effect(() =>
    context.cli.register(
      "status",
      async (invocation): Promise<CliResult> => {
        const status = await maybeHandleSlpStatus(context, invocation);
        if (status) return status;
        throw new CliError(
          "NO_ACTIVE_TEAM",
          invocation.positionals[0]
            ? "status <work-id> reads SLP team work from a role pane; outside a running team use: maestro work show <id>"
            : "status reads a running SLP team from a role pane or the Hub room; for tracked work run: maestro work list",
        );
      },
      {
        description: "Show the SLP team readout: the Hub's teams, a seat's work, one work item or one ruling.",
        flags: { "--all": { description: "List DONE work instead of counting it." } },
        mutates: false,
        positionals: [{ name: "work-id", required: false }],
      },
    )
  );
  context.effect(() =>
    context.events.on<{ id: string }, DecisionSection | null>("decision.show", ({ id }) => {
      const slp = slpDecisionResult(context, id);
      return slp ? { data: { slpDecisions: slp.decisions }, label: "SLP rulings", text: slp.text } : null;
    })
  );
  context.effect(() =>
    context.events.on<{ home: string; invocation: CliInvocation }>("install.begin", async ({ home, invocation }) => {
      // The re-executed install inherits the flag, but the token is already written and stdin is spent.
      if (invocation.options["seat-token"] !== true || process.env.MAESTRO_INSTALL_REEXEC === "1") return;
      await writeSeatToken(home, await Bun.stdin.text());
    })
  );
  context.effect(() => context.events.on<InstallEvent, string>("install", (event) => installMachineFiles(event)));
  context.effect(() =>
    context.events.on<{ home: string; repo: string; retired: string[] }, string>("update", async ({ home, repo, retired }) => {
      await syncSeatFiles(home, repo, retired);
      return "";
    })
  );
  context.effect(() => context.events.on<{ home: string }, string[]>("uninstall", ({ home }) => removeMachineFiles(home)));
  context.effect(() => context.events.on<{ home: string }, string>("doctor", ({ home }) => seatTokenCheck(home)));
}

export function applySlp(context: PluginContext): void {
  const runtime = new HerdrSlpRuntime();
  registerCoreSeams(context);
  const recipes = context.recipe as RecipeService | undefined;
  if (recipes) {
    context.effect(() =>
      recipes.register({
        name: "slp",
        description: "Know which SLP role a session holds (Supervisor, Lead, Peer) and what it owns.",
        body: () => readFileSync(join(import.meta.dir, "recipes", "slp.md"), "utf8"),
      })
    );
  }
  context.effect(() =>
    registerSessionCommand(
      context,
      "team start",
      (invocation): Promise<CliResult> => {
        // room d128: a seat's model is a launch-time choice. Each flag is
        // passed through unvalidated (the harness judges the name); only an
        // empty value is refused here, since it would launch `--model ""`.
        const models: PackModels = {};
        for (const [flag, seat] of [
          ["lead-model", "lead"],
          ["peer-model", "peer"],
        ] as const) {
          const value = stringOption(invocation, flag);
          if (value === undefined) continue;
          if (value.trim() === "") {
            throw new CliError("INVALID_OPTION", `--${flag} needs a model name; the harness validates it, maestro passes it through`);
          }
          models[seat] = value;
        }
        return startTeam(
          context,
          runtime,
          requiredPosition(invocation, 0, "project"),
          requiredPosition(invocation, 1, "objective"),
          models,
        );
      },
      {
        description: "Start or restore one SLP team generation: one Lead pane plus the runtime pane; the Hub Supervisor reviews the Lead's work (room d117).",
        flags: {
          "--lead-model": {
            description: "Launch the Lead with this model (`claude --model` / `codex --model` on its launch line); the lead seat file's model is the default; recorded on the team row (room d128).",
            value: true,
          },
          "--peer-model": {
            description: "Launch every Peer this generation opens with this model unless work add --model names another; recorded on the team row (room d128); name a model the peer seat's harness accepts, or its pane dies on the harness's own error.",
            value: true,
          },
        },
        positionals: [
          { name: "project", required: true },
          { name: "objective", required: true },
        ],
        rootDescription: "Run the simplified supervised-team lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "team stop",
      (invocation): Promise<CliResult> => stopTeam(context, runtime, invocation),
      {
        description: "Stop one complete SLP team, or abandon unfinished work from Hub.",
        flags: {
          "--emergency": { description: "Use Hub owner authority and abandon unfinished work." },
          "--reason": {
            description:
              "The closing report shown on the Hub ledger (w722); with --emergency, why unfinished work is abandoned.",
            value: true,
          },
        },
        positionals: [{ name: "team-id", required: true }],
        rootDescription: "Run the simplified supervised-team lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work take",
      (invocation): CliResult =>
        takeWork(context, requiredPosition(invocation, 0, "work id")),
      {
        description: "Take assigned OPEN work or reviewer-granted RETURNED SLP work.",
        positionals: [{ name: "work-id", required: true }],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work return",
      (invocation): Promise<CliResult> =>
        returnWork(
          context,
          requiredPosition(invocation, 0, "work id"),
          requiredPosition(invocation, 1, "return body"),
        ),
      {
        description: "Return ACTIVE SLP work with its bounded result body.",
        positionals: [
          { name: "work-id", required: true },
          { name: "body", required: true },
        ],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(
      context,
      "work accept",
      (invocation): Promise<CliResult> =>
        acceptWork(
          context,
          requiredPosition(invocation, 0, "work id"),
          stringOption(invocation, "outcome") ?? "accepted",
        ),
      {
        description: "Accept RETURNED SLP work at the reviewer boundary.",
        flags: {
          "--outcome": {
            description: "Record accepted or cancelled; cancellation may close OPEN work.",
            value: true,
          },
        },
        positionals: [{ name: "work-id", required: true }],
        rootDescription: "Move supervised work through its four-state lifecycle.",
      },
    ),
  );
  // Hub d96/d97: the plugin entrypoints and the runtime's own readout. None
  // is an SLP operation; Herdr launches the first two from the manifest.
  context.effect(() =>
    context.cli.register(
      "slp runtime",
      async (): Promise<CliResult> => {
        const exitCode = await runSlpRuntime(runtimeConfigFromEnvironment());
        if (exitCode !== 0) throw new CliError("RUNTIME_EXIT", `maestro slp runtime exited with ${exitCode}`);
        return { data: { exitCode }, text: "runtime stopped" };
      },
      {
        description: "Run the team runtime pane for MAESTRO_SLP_TEAM and MAESTRO_SLP_GENERATION (opened by team start through the maestro Herdr plugin).",
        rootDescription: "Maestro's Herdr plugin entrypoints and the runtime readout.",
      },
    ),
  );
  context.effect(() =>
    context.cli.register(
      "slp restore",
      (): Promise<CliResult> => runSlpRestore(),
      { description: "Herdr startup hook: reopen the runtime pane of every RUNNING generation whose role panes survived." },
    ),
  );
  context.effect(() =>
    context.cli.register(
      "slp status",
      (): Promise<CliResult> => {
        const team = activeLocalTeam(context);
        if (!team) throw new CliError("NO_ACTIVE_TEAM", "no running SLP team is bound to this workspace");
        return slpRuntimeStatus(team.project_path, team.team_id, team.generation);
      },
      { description: "Read the runtime's pending wakes for the running generation.", mutates: false },
    ),
  );
  // w740 (d898): bundle open says a design has begun; the SLP plugin owns
  // the push, so the bundle plugin only announces the event.
  context.effect(() =>
    context.events.on<{ bundleId: string; workIds: string[] }>(
      "bundle.open",
      async (opened, next) => {
        await pushDesignOpened(context, opened.bundleId, opened.workIds);
        return next();
      },
    ),
  );
  context.effect(() =>
    registerSessionCommand(context, "decide", (invocation): Promise<CliResult> => decide(context, invocation), {
      description: "Record one immutable settled decision.",
      flags: {
        "--replaces": { description: "Link the decision it replaces.", value: true },
        "--scope": {
          description: "Select owner or cross-team scope when acting as Hub Supervisor.",
          value: true,
        },
        "--why": { description: "Record why this choice is settled.", value: true },
        "--work": { description: "Link the decision to SLP work.", value: true },
      },
      positionals: [{ name: "choice", required: true }],
      rootDescription: "Record a settled SLP decision in one operation.",
    }),
  );
}
