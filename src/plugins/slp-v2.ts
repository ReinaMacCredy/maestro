import { createHash, randomUUID } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOption,
  type CliInvocation,
  type CliOptions,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { Store, resolveStoreLocation, tableExists } from "../kernel/store.ts";
import { resolveHomeDirectory } from "./home.ts";
import {
  composedPeerName,
  profileDigest,
  profileDirectories,
  resolveProfile,
  type Profile,
} from "./profiles.ts";
import { isRoom } from "./room.ts";
import {
  buildSlpTeamPlan,
  HerdrSlpRuntime,
  slpStopEnvironment,
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
import {
  openRuntimePane,
  recordRuntimePane,
  runSlpEvent,
  runSlpRestore,
  runSlpRuntime,
  runtimeConfigFromEnvironment,
  slpRuntimeStatus,
} from "./slp-attention.ts";

type WorkState = "OPEN" | "ACTIVE" | "RETURNED" | "DONE";
type LifecycleOperation = "START" | "STOP";
type LifecyclePhase = "RESERVED" | "RUNTIME_READY" | "COMMITTED";

interface PackProfiles {
  lead: string;
  peer: string;
  teamSupervisor: string;
}

// d91: a generation pins the pack plus the source bytes of every profile it
// referenced; a profile first used by a later work add is appended here.
interface PackConfiguration {
  profileDigests: Record<string, string>;
  profiles: PackProfiles;
}

// Generations started before pack version 3 stored models, not profiles; they
// resolve to the shipped seat names so status and stop can still build a plan.
function packConfiguration(json: string): PackConfiguration {
  const parsed = JSON.parse(json) as Partial<PackConfiguration>;
  const profiles = parsed.profiles && typeof parsed.profiles === "object" ? parsed.profiles : null;
  return {
    profileDigests: parsed.profileDigests ?? {},
    profiles: {
      lead: profiles?.lead ?? "lead",
      peer: profiles?.peer ?? "peer",
      teamSupervisor: profiles?.teamSupervisor ?? "team-supervisor",
    },
  };
}

function seatLaunch(profile: Profile): SeatLaunch {
  return {
    autocompact: profile.frontmatter.autocompact ?? null,
    harness: profile.frontmatter.harness,
    profile: profile.name,
  };
}

function seatDirectories(projectPath: string): string[] {
  return profileDirectories(projectPath, resolveHomeDirectory());
}

async function requireProfile(name: string, projectPath: string, marker: string): Promise<Profile> {
  const directories = seatDirectories(projectPath);
  const profile = await resolveProfile(name, directories);
  if (!profile) {
    throw new CliError(
      "PROFILE_NOT_FOUND",
      `${marker} names profile ${name}, which is not in ${directories.join(", ")}`,
      { directories, profile: name },
    );
  }
  return profile;
}

interface ResolvedSeats {
  lead: SeatLaunch;
  profileDigests: Record<string, string>;
  teamSupervisor: SeatLaunch;
}

async function resolveSeats(profiles: PackProfiles, projectPath: string): Promise<ResolvedSeats> {
  const teamSupervisor = await requireProfile(
    profiles.teamSupervisor,
    projectPath,
    "the team-supervisor marker",
  );
  const lead = await requireProfile(profiles.lead, projectPath, "the lead marker");
  const peer = await requireProfile(profiles.peer, projectPath, "the peer marker or --peer-profile");
  const profileDigests: Record<string, string> = {};
  for (const profile of [teamSupervisor, lead, peer]) profileDigests[profile.name] = profileDigest(profile);
  return { lead: seatLaunch(lead), profileDigests, teamSupervisor: seatLaunch(teamSupervisor) };
}

// Status and stop need only names and labels; a profile deleted since the
// generation started must not stop the team from being inspected or closed.
async function seatLaunchOrDefault(name: string, projectPath: string): Promise<SeatLaunch> {
  const profile = await resolveProfile(name, seatDirectories(projectPath));
  return profile ? seatLaunch(profile) : { autocompact: null, harness: "codex", profile: name };
}

async function planForTeam(team: {
  configuration_json: string;
  generation: number;
  project_path: string;
  team_id: string;
}): Promise<SlpTeamPlan> {
  const { profiles } = packConfiguration(team.configuration_json);
  return buildSlpTeamPlan({
    generation: team.generation,
    lead: await seatLaunchOrDefault(profiles.lead, team.project_path),
    projectPath: team.project_path,
    teamId: team.team_id,
    teamSupervisor: await seatLaunchOrDefault(profiles.teamSupervisor, team.project_path),
  });
}

// A4: a referenced profile edited mid-generation is refused on the next op
// with the same error shape as a pack edit, naming the profile.
async function requireProfilesUnchanged(
  configuration: PackConfiguration,
  team: { generation: number; project_path: string; team_id: string },
): Promise<void> {
  const directories = seatDirectories(team.project_path);
  for (const [name, digest] of Object.entries(configuration.profileDigests)) {
    const profile = await resolveProfile(name, directories);
    const actual = profile ? profileDigest(profile) : null;
    if (actual === digest) continue;
    throw new CliError(
      "SLP_SNAPSHOT_CHANGED",
      `running generation ${team.team_id}:g${team.generation} must keep its pinned profile ${name}${actual ? "" : " (now missing)"}`,
      { actual, expected: digest, profile: name },
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

const packSource = join(import.meta.dir, "resources", "SLP.md");

const retiredSlpOperations = new Map<string, string>([
  ["attention", "status"],
  ["brief", "status"],
  ["decision", "decide"],
  ["dispatch", "work add, work take, or work return"],
  ["handback", "work return or work accept"],
  ["ready", "status"],
  ["team advise", "decide"],
  ["team await-ready", "status"],
  ["team bind", "team start"],
  ["team health", "status"],
  ["team open", "team start"],
  ["team reconcile", "status or team start"],
  ["team review", "work note or work accept"],
  ["team status", "status"],
  ["work done", "work return then work accept"],
  ["work list", "status"],
  ["work reclaim", "work take"],
  ["work release", "work return"],
  ["work reopen", "work take"],
  ["work repair", "work note"],
  ["work show", "status <work-id>"],
  ["work start", "work take"],
]);

function canonicalCheckoutRoot(cwd: string): string {
  return realpathSync.native(resolveStoreLocation(cwd).root);
}

function adoptedSlpProjectAt(cwd: string): boolean {
  const location = resolveStoreLocation(cwd);
  const path = location.path;
  if (!existsSync(path)) return false;
  const store = new Store(path, { readonly: true });
  try {
    return tableExists(store, "slp_local_teams") &&
      store.database
        .query<{ present: number }, [string]>(
          "SELECT 1 AS present FROM slp_local_teams WHERE project_path = ? LIMIT 1",
        )
        .get(canonicalCheckoutRoot(cwd))?.present === 1;
  } finally {
    store.close();
  }
}

function retiredReplacement(command: string): string | null {
  const matches = [...retiredSlpOperations.entries()]
    .filter(([retired]) => command === retired || command.startsWith(`${retired} `))
    .sort(([left], [right]) => right.length - left.length);
  return matches[0]?.[1] ?? null;
}

function rejectRetiredSlp(command: string, replacement: string): never {
  throw new CliError(
    "SLP_V2_CUTOVER",
    `${command} was retired by SLP v2; use maestro ${replacement}`,
    { command, replacement },
  );
}

export function slpV2CliOptions(cwd = process.cwd()): CliOptions {
  return {
    beforeInvoke(command) {
      const replacement = retiredReplacement(command);
      if (!replacement || !adoptedSlpProjectAt(cwd)) return;
      const location = resolveStoreLocation(cwd);
      if (!existsSync(location.path)) return;
      const paneId = currentPaneId();
      if (!paneId) return;
      const store = new Store(location.path, { readonly: true });
      try {
        if (!tableExists(store, "slp_local_teams") || !tableExists(store, "slp_local_roles")) return;
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
        if (activeRole) rejectRetiredSlp(command, replacement);
      } finally {
        store.close();
      }
    },
    beforeUnknown(args) {
      const command = args.slice(0, 3).join(" ");
      const replacement = retiredReplacement(command);
      if (replacement) rejectRetiredSlp(command, replacement);
    },
  };
}

export async function defaultSlpPack(): Promise<Uint8Array> {
  return new Uint8Array(await readFile(packSource));
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
      role TEXT NOT NULL CHECK(role IN ('team-supervisor', 'lead', 'peer', 'observer')),
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
      created_at TEXT NOT NULL
    );
  `);
  store.ensureColumn(
    "slp_team_roles",
    "instance_id",
    "ALTER TABLE slp_team_roles ADD COLUMN instance_id TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_lifecycle_operations",
    "reason",
    "ALTER TABLE slp_lifecycle_operations ADD COLUMN reason TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_team_roles",
    "pack_digest",
    "ALTER TABLE slp_team_roles ADD COLUMN pack_digest TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_team_roles",
    "brief_digest",
    "ALTER TABLE slp_team_roles ADD COLUMN brief_digest TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_team_roles",
    "ready_challenge",
    "ALTER TABLE slp_team_roles ADD COLUMN ready_challenge TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_team_roles",
    "profile",
    "ALTER TABLE slp_team_roles ADD COLUMN profile TEXT NOT NULL DEFAULT ''",
  );
  widenRoleCheck(store, "slp_team_roles");
}

const ROLE_TABLE_COLUMNS = [
  "team_id", "generation", "role", "name", "pane_id", "workspace_id", "instance_id",
  "pack_digest", "brief_digest", "ready_challenge", "profile", "created_at",
].join(", ");

// d762 widened the role CHECK to admit 'observer'; the seat is gone (Hub d97,
// d98) but the constraint stays so rows from those generations still load.
// Tables created before the widening are rebuilt once, after ensureColumn has
// brought the old table up to the full column set.
function widenRoleCheck(store: Store, table: "slp_team_roles" | "slp_local_roles"): void {
  const sql = store.database
    .query<{ sql: string }, [string]>(
      "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get(table)?.sql ?? "";
  if (sql === "" || sql.includes("'observer'")) return;
  const foreignKey = table === "slp_team_roles"
    ? ",\n      FOREIGN KEY(team_id, generation) REFERENCES slp_teams(team_id, generation)"
    : "";
  store.migrate(`
    BEGIN IMMEDIATE;
    ALTER TABLE ${table} RENAME TO ${table}_legacy;
    CREATE TABLE ${table} (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('team-supervisor', 'lead', 'peer', 'observer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      instance_id TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      brief_digest TEXT NOT NULL,
      ready_challenge TEXT NOT NULL,
      profile TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name)${foreignKey}
    );
    INSERT INTO ${table} (${ROLE_TABLE_COLUMNS})
      SELECT ${ROLE_TABLE_COLUMNS} FROM ${table}_legacy;
    DROP TABLE ${table}_legacy;
    COMMIT;
  `);
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
      PRIMARY KEY(team_id, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_local_roles (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('team-supervisor', 'lead', 'peer', 'observer')),
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
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_rework_grants (
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      return_revision INTEGER NOT NULL CHECK(return_revision > 0),
      reviewer TEXT NOT NULL,
      granted_at TEXT NOT NULL,
      consumed_at TEXT,
      PRIMARY KEY(work_id, return_revision)
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
    CREATE TABLE IF NOT EXISTS slp_stop_grants (
      token TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      requested_by TEXT NOT NULL,
      owner_pid INTEGER,
      reason TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      UNIQUE(team_id, generation)
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
  store.ensureColumn(
    "slp_local_teams",
    "configuration_json",
    "ALTER TABLE slp_local_teams ADD COLUMN configuration_json TEXT NOT NULL DEFAULT '{}'",
  );
  store.ensureColumn(
    "slp_work",
    "return_revision",
    "ALTER TABLE slp_work ADD COLUMN return_revision INTEGER NOT NULL DEFAULT 0",
  );
  store.ensureColumn(
    "slp_work",
    "abandoned_at",
    "ALTER TABLE slp_work ADD COLUMN abandoned_at TEXT",
  );
  store.ensureColumn(
    "slp_work",
    "abandoned_by",
    "ALTER TABLE slp_work ADD COLUMN abandoned_by TEXT",
  );
  store.ensureColumn(
    "slp_work",
    "abandonment_reason",
    "ALTER TABLE slp_work ADD COLUMN abandonment_reason TEXT",
  );
  store.ensureColumn(
    "slp_lifecycle_operations",
    "reason",
    "ALTER TABLE slp_lifecycle_operations ADD COLUMN reason TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_stop_grants",
    "owner_pid",
    "ALTER TABLE slp_stop_grants ADD COLUMN owner_pid INTEGER",
  );
  store.ensureColumn(
    "slp_stop_grants",
    "reason",
    "ALTER TABLE slp_stop_grants ADD COLUMN reason TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_local_roles",
    "instance_id",
    "ALTER TABLE slp_local_roles ADD COLUMN instance_id TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_local_roles",
    "pack_digest",
    "ALTER TABLE slp_local_roles ADD COLUMN pack_digest TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_local_roles",
    "brief_digest",
    "ALTER TABLE slp_local_roles ADD COLUMN brief_digest TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_local_roles",
    "ready_challenge",
    "ALTER TABLE slp_local_roles ADD COLUMN ready_challenge TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_work_entries",
    "flag",
    "ALTER TABLE slp_work_entries ADD COLUMN flag TEXT",
  );
  store.ensureColumn(
    "slp_local_roles",
    "profile",
    "ALTER TABLE slp_local_roles ADD COLUMN profile TEXT NOT NULL DEFAULT ''",
  );
  store.ensureColumn(
    "slp_local_teams",
    "runtime_pane_id",
    "ALTER TABLE slp_local_teams ADD COLUMN runtime_pane_id TEXT NOT NULL DEFAULT ''",
  );
  widenRoleCheck(store, "slp_local_roles");
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

// d91/d98: pack version 3 names one profile per seat; the version-2 model
// markers and the Observer are refused by name so a migration is one message.
function requirePackV3(pack: string): string {
  const version = packVersion(pack);
  if (version !== "3" || /<!-- slp:model:/.test(pack)) {
    throw new CliError(
      "INVALID_SLP_PACK",
      `SLP.md is version ${version}; version 3 replaces every <!-- slp:model:<seat>=<harness>:<model> --> marker with <!-- slp:profile:<seat>=<name> --> for team-supervisor, lead and peer and moves the seat sections into profile files; maestro install rewrites the shipped copy under ~/.maestro/runtime, then migrate the Hub copy by hand`,
      { version },
    );
  }
  if (/<!-- slp:(?:profile|model):observer=|<!-- slp:role:observer:begin -->/.test(pack)) {
    throw new CliError(
      "INVALID_SLP_PACK",
      "SLP.md still carries an Observer marker or section; the Observer seat was removed (Hub d97, d98), delete it",
    );
  }
  return version;
}

function packProfile(pack: string, role: SlpRole): string {
  const value = new RegExp(`<!-- slp:profile:${role}=([a-z0-9-]+) -->`).exec(pack)?.[1];
  if (!value) {
    throw new CliError(
      "INVALID_SLP_PACK",
      `SLP.md is missing the ${role} profile marker <!-- slp:profile:${role}=<name> -->`,
    );
  }
  return value;
}

// d90: the mandate is the seat's rendered profile, so the post-open prompt
// carries only team, generation, instance and the ready challenge; the shared
// contract in every profile tells the seat how to answer it.
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
    ["team-supervisor", contract("team-supervisor")],
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

function nextWorkId(store: Store): string {
  const ids = store.database
    .query<{ id: string }, []>("SELECT id FROM slp_work")
    .all()
    .map((row) => row.id);
  if (tableExists(store, "slp_lifecycle_operations")) {
    ids.push(
      ...store.database
        .query<{ id: string }, []>(
          `SELECT work_id AS id FROM slp_lifecycle_operations
           WHERE operation = 'START' AND work_id <> ''`,
        )
        .all()
        .map((row) => row.id),
    );
  }
  if (tableExists(store, "work")) {
    ids.push(...store.database.query<{ id: string }, []>("SELECT id FROM work").all().map((row) => row.id));
  }
  const maximum = ids.reduce((current, id) => {
    const value = /^w(\d+)$/.exec(id)?.[1];
    return value ? Math.max(current, Number(value)) : current;
  }, 0);
  return `w${maximum + 1}`;
}

function nextDecisionId(store: Store): string {
  const ids = store.database
    .query<{ id: string }, []>("SELECT id FROM slp_decisions")
    .all()
    .map((row) => row.id);
  if (tableExists(store, "decisions")) {
    ids.push(
      ...store.database.query<{ id: string }, []>("SELECT id FROM decisions").all().map((row) => row.id),
    );
  }
  const maximum = ids.reduce((current, id) => {
    const value = /^d(\d+)$/.exec(id)?.[1];
    return value ? Math.max(current, Number(value)) : current;
  }, 0);
  return `d${maximum + 1}`;
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

// d98: the only per-generation seat override is --peer-profile.
function changedPeerProfile(override: string | undefined, profiles: PackProfiles): boolean {
  return override !== undefined && override !== profiles.peer;
}

function reserveStart(
  store: Store,
  input: {
    configuration: PackConfiguration;
    digest: string;
    objective: string;
    peerProfile: string | undefined;
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
      const { profiles } = packConfiguration(running.configuration_json);
      if (running.objective !== input.objective || changedPeerProfile(input.peerProfile, profiles)) {
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
      const stopGrant = store.database
        .query<{ owner_pid: number | null; token: string }, [string, number]>(
          `SELECT token, owner_pid FROM slp_stop_grants
           WHERE team_id = ? AND generation = ?`,
        )
        .get(running.team_id, running.generation);
      if (stopGrant && lifecycleOwnerIsAlive(stopGrant.owner_pid)) {
        throw new CliError(
          "TEAM_STOP_IN_PROGRESS",
          `${running.team_id}:g${running.generation} is shutting down; runtime repair is fenced`,
        );
      }
      if (stopGrant) {
        store.database
          .query("DELETE FROM slp_stop_grants WHERE token = ?")
          .run(stopGrant.token);
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
      const { profiles } = packConfiguration(pending.configuration_json);
      const changed = pending.objective !== input.objective ||
        changedPeerProfile(input.peerProfile, profiles);
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
    const workId = nextWorkId(store);
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
           bound_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?)`,
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
  roles: readonly SlpRuntimeRole[],
): Promise<string> {
  if (team.runtime_pane_id && await runtime.paneAlive(team.runtime_pane_id)) return team.runtime_pane_id;
  const supervisor = roles.find((role) => role.role === "team-supervisor") ?? null;
  let paneId: string;
  try {
    paneId = await openRuntimePane(runtime.client, team, supervisor?.paneId ?? null);
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

async function startTeam(
  context: PluginContext,
  runtime: HerdrSlpRuntime,
  projectInput: string,
  objective: string,
  peerProfileOverride: string | undefined,
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
    const hubVersion = requirePackV3(hubPack);
    const hubDigest = createHash("sha256").update(hubPackBytes).digest("hex");
    const hubProfiles: PackProfiles = {
      lead: packProfile(hubPack, "lead"),
      peer: peerProfileOverride ?? packProfile(hubPack, "peer"),
      teamSupervisor: packProfile(hubPack, "team-supervisor"),
    };
    const hubSeats = await resolveSeats(hubProfiles, projectPath);
    const hubConfiguration: PackConfiguration = {
      profileDigests: hubSeats.profileDigests,
      profiles: hubProfiles,
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
          objective,
          peerProfile: peerProfileOverride,
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
        await requireProfilesUnchanged(configuration, running);
        await archivePack(roomRoot, snapshotBytes, running.pack_digest);
        const seats = await resolveSeats(configuration.profiles, projectPath);
        const plan = buildSlpTeamPlan({
          generation: running.generation,
          lead: seats.lead,
          projectPath,
          teamId: running.team_id,
          teamSupervisor: seats.teamSupervisor,
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
               AND role IN ('team-supervisor', 'lead')`,
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
            started.roles,
          );
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
      const seats = await resolveSeats(configuration.profiles, projectPath);
      const plan = buildSlpTeamPlan({
        generation: operation.generation,
        lead: seats.lead,
        projectPath,
        teamId: operation.team_id,
        teamSupervisor: seats.teamSupervisor,
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
        roles,
      );
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
  team_id: string;
  workspace_id: string;
}

interface SlpActor {
  name: string;
  role: SlpRole;
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
              configuration_json, pack_digest, workspace_id, runtime_pane_id
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

const ancestorPaneIdPattern = /(?:^|\s)HERDR_PANE_ID=(\S+)/g;

// Codex sometimes runs a role's shell commands without the HERDR_* variables
// while its own process still carries them (lab g9, 2026-09-02); the pane
// identity then comes from the nearest ancestor process that has it (d770).
// macOS ps hides the environment of Apple platform binaries (zsh, sh), so the
// walk passes through the shell and reads it from the agent process.
export function currentPaneId(): string | undefined {
  if (process.env.HERDR_PANE_ID) return process.env.HERDR_PANE_ID;
  let pid = process.ppid;
  for (let depth = 0; depth < 10 && Number.isInteger(pid) && pid > 1; depth += 1) {
    let environment: ReturnType<typeof Bun.spawnSync>;
    let parent: ReturnType<typeof Bun.spawnSync>;
    try {
      environment = Bun.spawnSync(["ps", "eww", "-o", "command=", "-p", String(pid)]);
      parent = Bun.spawnSync(["ps", "-o", "ppid=", "-p", String(pid)]);
    } catch {
      return undefined;
    }
    if (environment.exitCode !== 0 || parent.exitCode !== 0) return undefined;
    if (!environment.stdout || !parent.stdout) return undefined;
    // ps prints the environment after the arguments; the last match is the env.
    const matches = [...environment.stdout.toString().matchAll(ancestorPaneIdPattern)];
    const match = matches.at(-1);
    if (match) return match[1];
    pid = Number(parent.stdout.toString().trim());
  }
  return undefined;
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

function requireSlpWork(context: PluginContext, actor: SlpActor, id: string): SlpWorkRow {
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

function requireRunningGeneration(
  store: Store,
  team: ActiveLocalTeam,
  allowedStopToken: string | null = null,
): void {
  requireRunningState(store, team);
  if (!tableExists(store, "slp_stop_grants")) return;
  const grant = store.database
    .query<{ token: string }, [string, number]>(
      "SELECT token FROM slp_stop_grants WHERE team_id = ? AND generation = ?",
    )
    .get(team.team_id, team.generation);
  if (grant && grant.token !== allowedStopToken) {
    throw new CliError(
      "TEAM_STOP_IN_PROGRESS",
      `${team.team_id}:g${team.generation} is shutting down; retry after team stop finishes`,
    );
  }
  if (allowedStopToken === null && tableExists(store, "slp_lifecycle_operations")) {
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

function expectedReviewerRole(
  context: PluginContext,
  actor: SlpActor,
  work: SlpWorkRow,
): "team-supervisor" | "lead" {
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
  return assignee.role === "lead" ? "team-supervisor" : "lead";
}

function requireWorkReviewer(
  context: PluginContext,
  actor: SlpActor,
  work: SlpWorkRow,
  operation: "accept" | "grant rework",
): void {
  const expectedReviewer = expectedReviewerRole(context, actor, work);
  if (actor.role !== expectedReviewer || actor.name === work.assigned_to) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      `${actor.role} cannot ${operation} for work assigned to ${work.assigned_to}`,
      { actor: actor.name, expectedReviewer },
    );
  }
}

function stopSuffix(stop: { emergency: boolean; reason: string } | null): string {
  if (!stop || stop.emergency || stop.reason === "") return "";
  return ` (supervisor): ${stop.reason}`;
}

function noticeSummary(body: string): string {
  const line = body.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > 160 ? `${line.slice(0, 157)}...` : line;
}

// d753/d760: the store is the truth; the pushed line is only the wake-up.
async function pushNotice(
  projectPath: string,
  fromRole: SlpRole,
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

function rolePaneName(context: PluginContext, actor: SlpActor, role: SlpRole): string | null {
  return context.store.database
    .query<{ name: string }, [string, number, string]>(
      `SELECT name FROM slp_local_roles
       WHERE team_id = ? AND generation = ? AND role = ?
       ORDER BY created_at LIMIT 1`,
    )
    .get(actor.team.team_id, actor.team.generation, role)?.name ?? null;
}

function readSlpWork(context: PluginContext, actor: SlpActor, id: string): SlpWorkRow {
  return requireSlpWork(context, actor, id);
}

function recordProjectActivity(
  context: PluginContext,
  actor: SlpActor,
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

// d91: a profile first used by work add joins the generation's pin in both
// stores, inside the same transaction as the Peer row.
function appendPinnedProfile(
  store: Store,
  team: { generation: number; team_id: string },
  pinned: { digest: string; name: string },
): void {
  for (const table of ["slp_local_teams", "slp_room.slp_teams"]) {
    const current = store.database
      .query<{ configuration_json: string }, [string, number]>(
        `SELECT configuration_json FROM ${table} WHERE team_id = ? AND generation = ?`,
      )
      .get(team.team_id, team.generation);
    if (!current) throw new CliError("NO_ACTIVE_TEAM", `no running SLP team row in ${table}`);
    const configuration = packConfiguration(current.configuration_json);
    configuration.profileDigests[pinned.name] = pinned.digest;
    store.database
      .query(`UPDATE ${table} SET configuration_json = ? WHERE team_id = ? AND generation = ?`)
      .run(JSON.stringify(configuration), team.team_id, team.generation);
  }
}

export async function maybeHandleSlpWorkAdd(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  if (isRoom(context.store.database) && invocation.options.to !== undefined) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      "Hub Supervisor cannot assign team work directly; communicate through Team Supervisor",
    );
  }
  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["team-supervisor", "lead"]);
  requireRunningGeneration(context.store, actor.team);
  const objective = requiredPosition(invocation, 0, "work objective");
  const requestedTarget = stringOption(invocation, "to");
  const profileOption = stringOption(invocation, "profile");
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
  let pinnedProfile: { digest: string; name: string } | null = null;
  const runtime = new HerdrSlpRuntime();
  if (actor.role === "team-supervisor") {
    if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "the team has no Lead");
    if (requestedTarget && requestedTarget !== "lead" && requestedTarget !== assignee.name) {
      throw new CliError("ROLE_FORBIDDEN", "Team Supervisor work is assigned to the Lead");
    }
    if (profileOption !== undefined) {
      throw new CliError("INVALID_OPTION", "--profile applies to Lead work add --to <peer>");
    }
  } else {
    if (!requestedTarget) {
      throw new CliError("MISSING_ARGUMENT", "Lead work add requires --to <peer>");
    }
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
    await requireProfilesUnchanged(configuration, actor.team);
    // d91/d98: --profile wins; a --to of the form peer-<name> naming a profile
    // uses it; otherwise the generation's peer profile applies.
    const directories = seatDirectories(actor.team.project_path);
    let peerProfileName = profileOption ?? configuration.profiles.peer;
    if (profileOption === undefined && requestedTarget.startsWith("peer-")) {
      const node = requestedTarget.slice("peer-".length);
      if (await resolveProfile(node, directories)) peerProfileName = node;
      else if (await resolveProfile(requestedTarget, directories)) peerProfileName = requestedTarget;
    }
    const peerProfile = await requireProfile(
      peerProfileName,
      actor.team.project_path,
      profileOption === undefined ? "the peer profile" : "--profile",
    );
    const renderedProfile = peerProfileName === "peer" ? "peer" : composedPeerName(peerProfileName);
    const knownPeer = roles.find((role) => role.name === peerName) ?? null;
    if (knownPeer && knownPeer.profile !== "" && knownPeer.profile !== renderedProfile) {
      throw new CliError(
        "PEER_PROFILE_MISMATCH",
        `${peerName} already runs profile maestro-${knownPeer.profile}; ${requestedTarget} cannot switch to maestro-${renderedProfile}, pick another Peer name`,
        { peer: peerName, profile: knownPeer.profile, requested: renderedProfile },
      );
    }
    if (configuration.profileDigests[peerProfileName] === undefined) {
      pinnedProfile = { digest: profileDigest(peerProfile), name: peerProfileName };
    }
    plan = await planForTeam(actor.team);
    const peerPlan: SlpRolePlan = {
      autocompact: peerProfile.frontmatter.autocompact ?? null,
      kind: peerProfile.frontmatter.harness,
      label: `slp:${actor.team.team_id}:g${actor.team.generation}:peer:${peerName}`,
      name: peerName,
      profile: renderedProfile,
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
    assignee = {
      name: ensured.role.name,
      pane_id: ensured.role.paneId,
      profile: ensured.role.profile,
      role: ensured.role.role,
      workspace_id: ensured.role.workspaceId,
      instance_id: ensured.role.instanceId,
      pack_digest: ensured.role.packDigest,
      brief_digest: ensured.role.briefDigest,
      ready_challenge: ensured.role.readyChallenge,
    };
  }
  if (!assignee) throw new CliError("RUNTIME_INCOMPLETE", "work assignee is unavailable");

  const now = new Date().toISOString();
  let attachedRoom = false;
  let id = "";
  try {
    if (actor.role === "lead") {
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
    if (actor.role === "lead") {
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
      if (pinnedProfile) appendPinnedProfile(context.store, actor.team, pinnedProfile);
    }
    id = nextWorkId(context.store);
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

export async function maybeHandleSlpWorkNote(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  if (!requireActiveOrLegacy(context)) return null;
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["team-supervisor", "lead", "peer"]);
  requireRunningGeneration(context.store, actor.team);
  const id = requiredPosition(invocation, 0, "work id");
  const body = requiredPosition(invocation, 1, "note body");
  const work = requireSlpWork(context, actor, id);
  const rework = invocation.options.rework === true;
  const blocked = invocation.options.blocked === true;
  if (rework && blocked) {
    throw new CliError("INVALID_OPTION", "--blocked and --rework are separate notes");
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
  const flag = blocked ? "blocked" : null;
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
  // d761: the blocked note wakes the seat above the actor, never the reviewer
  // of the item, because the one who is stuck is the one escalating.
  if (blocked && actor.role === "team-supervisor") {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      "supervisor",
      `${id} BLOCKED`,
      `${body} in ${actor.team.team_id} g${actor.team.generation}`,
      "maestro status",
    );
  } else if (blocked) {
    await pushNotice(
      actor.team.project_path,
      actor.role,
      rolePaneName(context, actor, actor.role === "lead" ? "team-supervisor" : "lead"),
      `${id} BLOCKED`,
      body,
      `maestro status ${id}`,
    );
  }
  const kind = rework ? "rework grant" : blocked ? "blocked note" : "note";
  return {
    data: {
      note: { actor: actor.name, body, createdAt: now, flag, rework },
      work: workData(readSlpWork(context, actor, id)),
    },
    text: `${id} ${kind} by ${actor.name}: ${body}`,
  };
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
    rolePaneName(context, actor, expectedReviewerRole(context, actor, current)),
    `${id} RETURNED`,
    body,
    `maestro status ${id}`,
  );
  return { data: { work: workData(current) }, text: `${id} RETURNED by ${actor.name}` };
}

async function acceptWork(context: PluginContext, id: string, outcome: string): Promise<CliResult> {
  migrateProject(context.store);
  const actor = requireSlpActor(context, ["team-supervisor", "lead"]);
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
      "supervisor",
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

function resolveHubWorkTarget(context: PluginContext, reference: string): HubWorkTarget {
  migrateRoom(context.store);
  const separator = reference.indexOf(":");
  const qualifiedTeam = separator > 0 ? reference.slice(0, separator) : null;
  const workId = separator > 0 ? reference.slice(separator + 1) : reference;
  if (!workId) throw new CliError("INVALID_WORK_ID", "--work requires a work id");
  const teams = context.store.database
    .query<{
      generation: number;
      project_path: string;
      team_id: string;
    }, []>(
      `SELECT team_id, generation, project_path FROM slp_teams
       WHERE state = 'RUNNING'
       ORDER BY project_path, generation`,
    )
    .all()
    .filter((team) => qualifiedTeam === null || team.team_id === qualifiedTeam);
  const matches: HubWorkTarget[] = [];
  for (const team of teams) {
    let projectStore: Store;
    try {
      projectStore = new Store(resolveStoreLocation(team.project_path).path, { readonly: true });
    } catch (error) {
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
  if (matches.length === 0) {
    throw new CliError("NOT_FOUND", `SLP work not found from Hub: ${reference}`);
  }
  if (matches.length > 1) {
    throw new CliError(
      "AMBIGUOUS_WORK",
      `${workId} exists in multiple teams; qualify it as <team-id>:${workId}`,
      { candidates: matches.map((match) => `${match.teamId}:${match.workId}`) },
    );
  }
  return matches[0] as HubWorkTarget;
}

function decideHubWork(
  context: PluginContext,
  input: {
    choice: string;
    replaces: string | null;
    scope: string;
    target: HubWorkTarget;
    why: string;
  },
): CliResult {
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
    id = nextDecisionId(context.store);
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
  return { data: { decision }, text: `${id} [${input.scope}] ${input.choice}` };
}

function decide(context: PluginContext, invocation: CliInvocation): CliResult {
  const choice = requiredPosition(invocation, 0, "choice");
  const why = stringOption(invocation, "why")?.trim();
  if (!why) throw new CliError("MISSING_ARGUMENT", "decide requires --why <reason>");
  const local = activeLocalTeam(context);
  let actor: string;
  let teamId: string;
  let generation: number;
  let defaultScope: "technical" | "team" | "owner";
  let allowedScopes: readonly string[];
  if (local) {
    const role = requireSlpActor(context, ["team-supervisor", "lead"]);
    requireRunningGeneration(context.store, role.team);
    actor = role.name;
    teamId = role.team.team_id;
    generation = role.team.generation;
    defaultScope = role.role === "lead" ? "technical" : "team";
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
    return decideHubWork(context, {
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
    id = nextDecisionId(context.store);
    context.store.database
      .query(
        `INSERT INTO slp_decisions
          (id, team_id, generation, choice, why, scope, work_id, replaces_id,
           actor, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(id, teamId, generation, choice, why, scope, workId, replaces, actor, now);
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

interface StopProxyEnvironment {
  closeWorkspace: boolean;
  helperTabId: string;
  helperWorkspaceId: string;
  projectPath: string;
  token: string;
}

function stopProxyEnvironment(): StopProxyEnvironment | null {
  const token = process.env[slpStopEnvironment.token];
  if (!token) return null;
  const projectPath = process.env[slpStopEnvironment.project];
  const helperTabId = process.env[slpStopEnvironment.helperTab];
  const helperWorkspaceId = process.env[slpStopEnvironment.helperWorkspace];
  const closeWorkspace = process.env[slpStopEnvironment.closeWorkspace];
  if (
    !projectPath ||
    !helperTabId ||
    !helperWorkspaceId ||
    (closeWorkspace !== "0" && closeWorkspace !== "1")
  ) {
    throw new CliError("INVALID_STOP_GRANT", "incomplete SLP stop helper authority");
  }
  return {
    closeWorkspace: closeWorkspace === "1",
    helperTabId,
    helperWorkspaceId,
    projectPath,
    token,
  };
}

function localTeamState(store: Store, team: ActiveLocalTeam): "RUNNING" | "STOPPED" | null {
  return store.database
    .query<{ state: "RUNNING" | "STOPPED" }, [string, number]>(
      "SELECT state FROM slp_local_teams WHERE team_id = ? AND generation = ?",
    )
    .get(team.team_id, team.generation)?.state ?? null;
}

function clearStopGrant(store: Store, team: ActiveLocalTeam, token: string): void {
  if (!tableExists(store, "slp_stop_grants")) return;
  store.database
    .query(
      "DELETE FROM slp_stop_grants WHERE team_id = ? AND generation = ? AND token = ?",
    )
    .run(team.team_id, team.generation, token);
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

function issueStopGrant(
  store: Store,
  team: ActiveLocalTeam,
  actor: string,
  reason: string,
): string {
  migrateProject(store);
  const token = randomUUID();
  store.database.exec("BEGIN IMMEDIATE");
  try {
    requireRunningState(store, team);
    const unfinished = unfinishedWork(store, team);
    if (unfinished.length > 0) {
      throw new CliError(
        "TEAM_UNFINISHED",
        `${team.team_id} has unfinished work: ${unfinished.map((work) => `${work.id} [${work.state}]`).join(", ")}`,
        { unfinished },
      );
    }
    const existing = store.database
      .query<{ owner_pid: number | null; requested_by: string; token: string }, [string, number]>(
        `SELECT token, requested_by, owner_pid FROM slp_stop_grants
         WHERE team_id = ? AND generation = ?`,
      )
      .get(team.team_id, team.generation);
    if (existing && lifecycleOwnerIsAlive(existing.owner_pid)) {
      throw new CliError(
        "TEAM_STOP_IN_PROGRESS",
        `${team.team_id}:g${team.generation} is already being stopped by ${existing.requested_by}`,
      );
    }
    store.database
      .query("DELETE FROM slp_stop_grants WHERE team_id = ? AND generation = ?")
      .run(team.team_id, team.generation);
    store.database
      .query(
        `INSERT INTO slp_stop_grants
          (token, team_id, generation, requested_by, owner_pid, reason, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        token,
        team.team_id,
        team.generation,
        actor,
        process.pid,
        reason,
        new Date().toISOString(),
      );
    store.database.exec("COMMIT");
    return token;
  } catch (error) {
    try {
      store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
}

async function waitForStopped(
  store: Store,
  team: ActiveLocalTeam,
  token: string,
): Promise<void> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (localTeamState(store, team) === "STOPPED") return;
    const grant = store.database
      .query<{ token: string }, [string, number]>(
        "SELECT token FROM slp_stop_grants WHERE team_id = ? AND generation = ?",
      )
      .get(team.team_id, team.generation);
    if (!grant || grant.token !== token) {
      throw new CliError(
        "TEAM_STOP_INCOMPLETE",
        `${team.team_id}:g${team.generation} remains RUNNING; inspect status and retry team stop`,
      );
    }
    await Bun.sleep(50);
  }
  throw new CliError(
    "TEAM_STOP_PENDING",
    `${team.team_id}:g${team.generation} remains RUNNING while its stop helper finishes; inspect status before retrying`,
  );
}

async function requestNormalStop(
  context: PluginContext,
  runtime: HerdrSlpRuntime,
  actor: SlpActor,
  reason: string,
): Promise<CliResult> {
  const team = actor.team;
  const token = issueStopGrant(context.store, team, actor.name, reason);
  const cliEntry = process.argv[1];
  if (!cliEntry) {
    clearStopGrant(context.store, team, token);
    throw new CliError("RUNTIME_UNAVAILABLE", "cannot locate the running Maestro entrypoint");
  }
  try {
    await runtime.delegateStop(
      await stopPlan(team),
      dirname(dirname(team.room_store_path)),
      token,
      resolve(cliEntry),
    );
    await waitForStopped(context.store, team, token);
    await pushNotice(
      team.project_path,
      "team-supervisor",
      "supervisor",
      `${team.team_id} g${team.generation} STOPPED`,
      reason || "normal stop",
      "maestro status",
    );
    return stopResult(team, false);
  } catch (error) {
    if (localTeamState(context.store, team) === "STOPPED") return stopResult(team, false);
    if (!(error instanceof CliError && error.code === "TEAM_STOP_PENDING")) {
      try {
        clearStopGrant(context.store, team, token);
      } catch {}
    }
    throw error;
  }
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
    proxyToken: string | null;
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

    let actor = "hub-supervisor";
    let reason = input.reason;
    if (input.proxyToken) {
      requireRunningGeneration(store, team, input.proxyToken);
      const grant = store.database
        .query<{ reason: string; requested_by: string }, [string, number, string]>(
          `SELECT requested_by, reason FROM slp_stop_grants
           WHERE team_id = ? AND generation = ? AND token = ?`,
        )
        .get(team.team_id, team.generation, input.proxyToken);
      if (!grant) throw new CliError("INVALID_STOP_GRANT", "stop helper authority expired");
      actor = grant.requested_by;
      reason = grant.reason;
      const unfinished = unfinishedWork(store, team);
      if (unfinished.length > 0) {
        throw new CliError(
          "TEAM_UNFINISHED",
          `${team.team_id} has unfinished work: ${unfinished.map((work) => `${work.id} [${work.state}]`).join(", ")}`,
          { unfinished },
        );
      }
    } else {
      requireRunningState(store, team);
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
    store.database
      .query("DELETE FROM slp_stop_grants WHERE team_id = ? AND generation = ?")
      .run(current.team_id, current.generation);
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
  const emergencyReason = emergency
    ? requestedReason ?? "Hub Supervisor emergency stop"
    : "";
  const proxy = stopProxyEnvironment();
  if (!isRoom(context.store.database)) {
    if (proxy) throw new CliError("INVALID_STOP_GRANT", "stop helper must run from its Hub");
    if (emergency) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "--emergency is Hub Supervisor authority and must run from ~/maestro",
      );
    }
    const actor = requireSlpActor(context, ["team-supervisor"]);
    if (requestedTeam !== actor.team.team_id) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        `${actor.name} cannot stop ${requestedTeam}; current team is ${actor.team.team_id}`,
      );
    }
    return requestNormalStop(context, runtime, actor, requestedReason ?? "");
  }

  if (requestedReason !== undefined && !emergency) {
    throw new CliError("INVALID_OPTION", "--reason requires --emergency");
  }
  if (!emergency && !proxy) {
    throw new CliError(
      "ROLE_FORBIDDEN",
      "Hub Supervisor may stop a team only with --emergency; normal stop belongs to Team Supervisor",
    );
  }
  if (emergency && proxy) {
    throw new CliError("INVALID_STOP_GRANT", "stop helper cannot claim emergency authority");
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
  if (proxy && canonicalCheckoutRoot(proxy.projectPath) !== roomTeam.project_path) {
    throw new CliError("INVALID_STOP_GRANT", "stop helper project does not match its Hub team");
  }

  const projectStore = new Store(resolveStoreLocation(roomTeam.project_path).path);
  migrateProject(projectStore);
  const team = projectStore.database
    .query<ActiveLocalTeam, [string, number]>(
      `SELECT team_id, generation, room_store_path, project_path,
              configuration_json, pack_digest, workspace_id, runtime_pane_id
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
      if (proxy) throw new CliError("INVALID_STOP_GRANT", "stop helper generation is already stopped");
      await runtime.stop(await stopPlan(team), stopRoles(projectStore, team));
      context.sessions.record("team.stop.emergency");
      return {
        data: {
          emergency: true,
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
        proxyToken: proxy?.token ?? null,
        reason: emergencyReason,
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
      if (proxy) throw new CliError("INVALID_STOP_GRANT", "stop helper generation is already stopped");
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
      if (proxy) {
        try {
          clearStopGrant(projectStore, team, proxy.token);
        } catch {}
      }
      throw error;
    }
    const committedEmergency = operation.emergency === 1;
    context.sessions.record(committedEmergency ? "team.stop.emergency" : "team.stop");
    return stopResult(team, committedEmergency);
  } finally {
    if (attachedRoom) projectStore.database.exec("DETACH DATABASE slp_room");
    projectStore.close();
    if (proxy) {
      try {
        await runtime.closeStopHelper(
          dirname(dirname(context.store.path)),
          proxy.helperTabId,
          proxy.helperWorkspaceId,
          proxy.closeWorkspace,
        );
      } catch {}
    }
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
       ORDER BY CASE role WHEN 'team-supervisor' THEN 0 WHEN 'lead' THEN 1 ELSE 2 END, name`,
    )
    .all(teamId, generation);
}

type SlpStatusRole = ReturnType<typeof roleRows>[number];

interface SlpNextStep {
  mayRun: string[];
  waitingOn: string | null;
}

function clipLine(text: string, limit: number): string {
  const line = text.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > limit ? `${line.slice(0, limit - 3)}...` : line;
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
  actor: SlpActor,
  roles: SlpStatusRole[],
  work: SlpWorkRow,
  grantOpen: boolean,
): SlpNextStep {
  const assigneeRole = roles.find((role) => role.name === work.assigned_to)?.role ?? "peer";
  const reviewerRole: SlpRole = assigneeRole === "lead" ? "team-supervisor" : "lead";
  const reviewerName = roles.find((role) => role.role === reviewerRole)?.name ?? reviewerRole;
  const reviewing = actor.role === reviewerRole && actor.name !== work.assigned_to;
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

function nextLine(work: SlpWorkRow, step: SlpNextStep): string {
  if (work.state === "DONE") return `next: none (${work.acceptance_outcome ?? "done"})`;
  if (step.waitingOn === null) return `next: ${step.mayRun.join(" | ")}`;
  const optional = step.mayRun.length > 0 ? `; may run: ${step.mayRun.join(" | ")}` : "";
  return `next: waiting on ${step.waitingOn}${optional}`;
}

function workLine(work: SlpWorkRow, step: SlpNextStep): string {
  const marker = work.state !== "DONE" && step.waitingOn === null ? "*" : " ";
  return `${marker} ${work.id} ${work.state} ${work.created_by} -> ${work.assigned_to}: ${
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

export async function maybeHandleSlpStatus(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult | null> {
  const requestedWork = invocation.positionals[0] ?? null;
  if (isRoom(context.store.database)) {
    if (requestedWork) {
      throw new CliError(
        "ROLE_FORBIDDEN",
        "Hub Supervisor does not manage project work directly; run status <work-id> in the team workspace",
      );
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
      const projectStore = new Store(resolveStoreLocation(row.project_path).path, { readonly: true });
      try {
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
        missingPanes,
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
    return {
      data: { teams },
      text: teams.length === 0
        ? "no SLP teams"
        : teams.map((team) =>
          `${team.teamId} g${team.generation} ${team.state}${stopSuffix(team.stop as { emergency: boolean; reason: string } | null)}; runtime pane ${team.runtimePane}; missing ${(team.missingPanes as string[]).join(", ") || "none"}`
        ).join("\n"),
    };
  }

  if (!requireActiveOrLegacy(context)) return null;
  const actor = requireSlpActor(context, ["team-supervisor", "lead", "peer"]);
  const roles = roleRows(context.store, actor.team.team_id, actor.team.generation, true);
  if (requestedWork) {
    const work = requireSlpWork(context, actor, requestedWork);
    if (actor.role === "peer" && work.assigned_to !== actor.name) {
      throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${requestedWork}`);
    }
    if (
      actor.role === "lead" &&
      work.assigned_to !== actor.name &&
      work.created_by !== actor.name
    ) {
      throw new CliError("ROLE_FORBIDDEN", `${actor.name} cannot inspect ${requestedWork}`);
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
      .all(requestedWork);
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
      .all(requestedWork)
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
          .all(actor.team.team_id, actor.team.generation, requestedWork)
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
    return {
      data: {
        acceptance: entries.findLast((entry) => entry.kind === "ACCEPTANCE") ?? null,
        decisions,
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
        `${work.id} ${work.state} ${work.created_by} -> ${work.assigned_to}`,
        `revision: ${work.return_revision}`,
        `objective: ${clipLine(work.objective, 160)}`,
        latest
          ? `${latest.kind.toLowerCase()}${latest.flag ? ` [${latest.flag}]` : ""} by ${latest.actor}: ${
            clipLine(latest.body, 160)
          }`
          : "entries: none",
        decisionLine(decisions.map((decision) => ({ id: decision.id, workId: null }))),
        nextLine(work, nextStep(actor, roles, work, reworkGrantOpen(context, work))),
      ].join("\n"),
    };
  }

  const allWork = context.store.database
    .query<SlpWorkRow, [string, number]>(
      `SELECT * FROM slp_work WHERE team_id = ? AND generation = ? ORDER BY created_at, id`,
    )
    .all(actor.team.team_id, actor.team.generation);
  const scoped = allWork.filter((work) => {
    if (actor.role === "team-supervisor") return work.state === "ACTIVE" || work.state === "RETURNED";
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
      ? [`${done.length} DONE; --all to list`]
      : []),
    decisionLine(teamDecisionRefs(context, actor)),
  ].join("\n");
}

export const slpV2Plugin: BuiltInPlugin = {
  name: "slp-v2",
  apply(context) {
    const runtime = new HerdrSlpRuntime();
    context.effect(() =>
      registerSessionCommand(
        context,
        "team start",
        (invocation): Promise<CliResult> => {
          // d98: the retired model flags are refused by name so the caller
          // learns the replacement instead of an unknown-flag error.
          for (const [flag, seat] of [
            ["peer-model", "peer"],
            ["lead-model", "lead"],
            ["supervisor-model", "team-supervisor"],
          ] as const) {
            if (invocation.options[flag] !== undefined) {
              throw new CliError(
                "RETIRED_FLAG",
                `--${flag} was retired by pack version 3 (Hub d98): a Peer variant is team start --peer-profile <name>; the Team Supervisor and Lead change through a shadowing profile file at ~/maestro/profiles/${seat}.md`,
                { flag: `--${flag}` },
              );
            }
          }
          return startTeam(
            context,
            runtime,
            requiredPosition(invocation, 0, "project"),
            requiredPosition(invocation, 1, "objective"),
            stringOption(invocation, "peer-profile"),
          );
        },
        {
          description: "Start or restore one supervised SLP team generation.",
          flags: {
            "--peer-profile": {
              description: "Override the Workspace Pack peer profile for this generation (recorded on the team row).",
              value: true,
            },
            "--lead-model": { hidden: true, value: true },
            "--peer-model": { hidden: true, value: true },
            "--supervisor-model": { hidden: true, value: true },
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
                "Team Supervisor: the closing report shown to the Hub. Hub: why unfinished work is abandoned by emergency stop.",
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
    // is an SLP operation; Herdr launches the first three from the manifest.
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
        "slp event",
        (): Promise<CliResult> => runSlpEvent(),
        { description: "Herdr event hook: record a role pane loss when no runtime is subscribed." },
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
    context.effect(() =>
      registerSessionCommand(context, "decide", (invocation): CliResult => decide(context, invocation), {
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
  },
};
