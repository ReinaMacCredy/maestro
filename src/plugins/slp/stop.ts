import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import { CliError, requiredPosition, stringOption, type CliInvocation, type CliResult } from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { Store, resolveStoreLocation, tableExists } from "../../kernel/store.ts";
import { isRoom } from "../room.ts";
import { HerdrSlpRuntime } from "./runtime.ts";
import type { SlpRuntimeRole, SlpTeamPlan } from "./runtime-plan.ts";
import { type ActiveLocalTeam, requireRunningState, roleRows } from "./context.ts";
import { lifecycleOwnerIsAlive, lifecycleRow, releaseLifecycleOwner, type SlpLifecycleRow } from "./lifecycle.ts";
import { migrateProject, migrateRoom } from "./migrations.ts";
import { planForTeam } from "./pack.ts";
import { withImmediateTransaction, type WorkState } from "./store.ts";

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

export async function stopTeam(
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
