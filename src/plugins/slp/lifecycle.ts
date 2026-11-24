import { CliError } from "../../kernel/cli.ts";
import { Store } from "../../kernel/store.ts";
import type { SlpRuntimeRole } from "./runtime-plan.ts";
import { withImmediateTransaction } from "./store.ts";

type LifecycleOperation = "START" | "STOP";

type LifecyclePhase = "RESERVED" | "RUNTIME_READY" | "COMMITTED";

export interface SlpLifecycleRow {
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

export function lifecycleOwnerIsAlive(ownerPid: number | null): boolean {
  if (!ownerPid) return false;
  try {
    process.kill(ownerPid, 0);
    return true;
  } catch {
    return false;
  }
}

export function lifecycleRow(
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

export function runtimePaneIdOf(store: Store, teamId: string, generation: number): string {
  return store.database
    .query<{ runtime_pane_id: string }, [string, number]>(
      "SELECT runtime_pane_id FROM slp_local_teams WHERE team_id = ? AND generation = ?",
    )
    .get(teamId, generation)?.runtime_pane_id ?? "";
}

export function lifecycleRuntimeRoles(row: SlpLifecycleRow): SlpRuntimeRole[] | null {
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

export function updateLifecycleOwner(
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

export function releaseLifecycleOwner(
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

export function claimRunningStartRepair(
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

export function releaseRunningStartRepair(
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
