import { createHash, randomUUID } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError, type CliResult } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, resolveStoreLocation } from "../../kernel/store.ts";
import { isRoom } from "../room.ts";
import { updateEntry } from "../plugin-manager.ts";
import {
  buildSlpTeamPlan,
  type SlpAcknowledgedRole,
  type SlpRole,
  type SlpRuntimeRole,
  type SlpRuntimeStart,
} from "./runtime-plan.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import { currentPaneId } from "../session-telemetry.ts";
import { openRuntimePane, recordRuntimePane } from "./attention.ts";
import { pushNotice } from "./context.ts";
import {
  claimRunningStartRepair,
  lifecycleOwnerIsAlive,
  lifecycleRow,
  lifecycleRuntimeRoles,
  releaseLifecycleOwner,
  releaseRunningStartRepair,
  runtimePaneIdOf,
  type SlpLifecycleRow,
  updateLifecycleOwner,
} from "./lifecycle.ts";
import { migrateProject, migrateRoom } from "./migrations.ts";
import {
  archivePack,
  packConfiguration,
  type PackConfiguration,
  type PackModels,
  packVersion,
  requireSeatsUnchanged,
  resolveSeats,
  roleContracts,
} from "./pack.ts";
import { restoreSnapshot, teamIdForProject, toWork, withImmediateTransaction, type WorkState } from "./store.ts";

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

export async function startTeam(
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
