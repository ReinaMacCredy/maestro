import { createHash } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { Store, resolveStoreLocation } from "../kernel/store.ts";
import {
  HerdrTeamRuntime,
  TeamRuntimeError,
  type MissingPostcondition,
  type RuntimeEffect,
  type TeamInspection,
  type TeamPlan,
  type TeamRuntime,
} from "./team-runtime.ts";

export type TeamControlBoundary =
  | "bundle.close"
  | "dispatch.open"
  | "external.effect"
  | "handback.final"
  | "work.add"
  | "work.done"
  | "work.start";

export interface TeamBinding {
  bindingId: string;
  generation: number;
  projectRoot: string;
  projectStorePath: string;
  roomIdentity: string;
  roomStorePath: string;
  teamId: string;
  token: string;
}

export interface TeamControlResult {
  allowed: boolean;
  binding: TeamBinding | null;
  bound: boolean;
  receiptId: string | null;
  reason: string;
  verdict: "OPERABLE" | "REVIEW_HOLD" | "DRAINING" | "CLOSED" | null;
}

interface LocalBindingRow {
  binding_id: string;
  generation: number;
  project_root: string;
  project_store_path: string;
  room_identity: string;
  room_store_path: string;
  team_id: string;
  token: string;
}

interface RoomBindingRow extends LocalBindingRow {
  status: string;
}

interface TeamRow {
  generation: number;
  health: "READY" | "DEGRADED" | null;
  plan_json: string;
  resources_json: string;
  review: "CLEAR" | "REVIEW_REQUIRED";
  revision: number;
  stage: "STARTING" | "ACTIVE" | "STOPPING" | "STOPPED";
  team_id: string;
  verdict: "OPERABLE" | "REVIEW_HOLD" | "DRAINING" | "CLOSED";
}

function identity(root: string, storePath: string): string {
  return createHash("sha256")
    .update(JSON.stringify({ root: resolve(root), storePath: resolve(storePath) }))
    .digest("hex");
}

function canonicalPath(path: string): string {
  try {
    return realpathSync(path);
  } catch {
    return resolve(path);
  }
}

function deriveVerdict(row: Pick<TeamRow, "health" | "review" | "stage">): TeamRow["verdict"] {
  if (row.stage !== "ACTIVE") return "CLOSED";
  if (row.health === "DEGRADED") return "DRAINING";
  if (row.health === "READY" && row.review === "REVIEW_REQUIRED") return "REVIEW_HOLD";
  return row.health === "READY" ? "OPERABLE" : "CLOSED";
}

function json<T>(value: string): T {
  return JSON.parse(value) as T;
}

export function migrateTeamControlTables(store: Store, root: string): void {
  store.migrate(`
    CREATE TABLE IF NOT EXISTS team_store_identity (
      singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
      identity TEXT NOT NULL,
      root TEXT NOT NULL,
      store_path TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS team_project_bindings (
      binding_id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      room_identity TEXT NOT NULL,
      room_store_path TEXT NOT NULL,
      project_root TEXT NOT NULL,
      project_store_path TEXT NOT NULL,
      token TEXT NOT NULL,
      status TEXT NOT NULL,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      UNIQUE(team_id, generation, project_root)
    );
    CREATE TABLE IF NOT EXISTS team_local_bindings (
      team_id TEXT PRIMARY KEY,
      binding_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      room_identity TEXT NOT NULL,
      room_store_path TEXT NOT NULL,
      project_root TEXT NOT NULL,
      project_store_path TEXT NOT NULL,
      token TEXT NOT NULL,
      bound_at TEXT NOT NULL
    );
  `);
  if (store.readOnly) return;
  const storeIdentity = identity(root, store.path);
  const now = new Date().toISOString();
  store.database
    .query(
      `INSERT INTO team_store_identity (singleton, identity, root, store_path, created_at)
       VALUES (1, ?, ?, ?, ?)
       ON CONFLICT(singleton) DO NOTHING`,
    )
    .run(storeIdentity, resolve(root), resolve(store.path), now);
}

function roomIdentity(store: Store): string {
  const row = store.database
    .query<{ identity: string }, []>(
      "SELECT identity FROM team_store_identity WHERE singleton = 1",
    )
    .get();
  if (!row) throw new Error("team store identity is missing");
  return row.identity;
}

function fromLocal(row: LocalBindingRow): TeamBinding {
  return {
    bindingId: row.binding_id,
    generation: row.generation,
    projectRoot: row.project_root,
    projectStorePath: row.project_store_path,
    roomIdentity: row.room_identity,
    roomStorePath: row.room_store_path,
    teamId: row.team_id,
    token: row.token,
  };
}

export function bindProject(input: {
  generation: number;
  projectRoot: string;
  roomRoot: string;
  roomStore: Store;
  teamId: string;
}): TeamBinding {
  const projectLocation = resolveStoreLocation(input.projectRoot);
  const projectRoot = resolve(projectLocation.root);
  const projectStorePath = resolve(projectLocation.path);
  const roomStorePath = resolve(input.roomStore.path);
  const authoritativeIdentity = roomIdentity(input.roomStore);
  const existing = input.roomStore.database
    .query<RoomBindingRow, [string, number, string]>(
      `SELECT * FROM team_project_bindings
       WHERE team_id = ? AND generation = ? AND project_root = ?`,
    )
    .get(input.teamId, input.generation, projectRoot);
  const bindingId = existing?.binding_id ?? `b-${crypto.randomUUID()}`;
  const token = existing?.token ?? crypto.randomUUID();
  const now = new Date().toISOString();
  input.roomStore.database
    .query(
      `INSERT INTO team_project_bindings
        (binding_id, team_id, generation, room_identity, room_store_path,
         project_root, project_store_path, token, status, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED', ?, ?)
       ON CONFLICT(team_id, generation, project_root) DO UPDATE SET
         room_identity = excluded.room_identity,
         room_store_path = excluded.room_store_path,
         project_store_path = excluded.project_store_path,
         status = 'ATTEMPTED',
         updated_at = excluded.updated_at`,
    )
    .run(
      bindingId,
      input.teamId,
      input.generation,
      authoritativeIdentity,
      roomStorePath,
      projectRoot,
      projectStorePath,
      token,
      existing ? now : now,
      now,
    );

  const projectStore = projectStorePath === roomStorePath
    ? input.roomStore
    : new Store(projectStorePath);
  try {
    migrateTeamControlTables(projectStore, projectRoot);
    projectStore.database
      .query(
        `INSERT INTO team_local_bindings
          (team_id, binding_id, generation, room_identity, room_store_path,
           project_root, project_store_path, token, bound_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(team_id) DO UPDATE SET
           binding_id = excluded.binding_id,
           generation = excluded.generation,
           room_identity = excluded.room_identity,
           room_store_path = excluded.room_store_path,
           project_root = excluded.project_root,
           project_store_path = excluded.project_store_path,
           token = excluded.token,
           bound_at = excluded.bound_at`,
      )
      .run(
        input.teamId,
        bindingId,
        input.generation,
        authoritativeIdentity,
        roomStorePath,
        projectRoot,
        projectStorePath,
        token,
        now,
      );
  } finally {
    if (projectStore !== input.roomStore) projectStore.close();
  }
  input.roomStore.database
    .query(
      `UPDATE team_project_bindings
       SET status = 'ACTIVE', updated_at = ?
       WHERE binding_id = ? AND token = ?`,
    )
    .run(new Date().toISOString(), bindingId, token);
  return {
    bindingId,
    generation: input.generation,
    projectRoot,
    projectStorePath,
    roomIdentity: authoritativeIdentity,
    roomStorePath,
    teamId: input.teamId,
    token,
  };
}

function runtimeFailure(error: unknown): TeamInspection {
  const actual = error instanceof TeamRuntimeError
    ? { command: error.command, message: error.message, stderr: error.stderr ?? null }
    : { message: error instanceof Error ? error.message : String(error) };
  const missing: MissingPostcondition[] = [{
    actual,
    code: "runtime.unavailable",
    expected: "fresh TeamRuntime inspection",
    resource: "runtime",
  }];
  return {
    actual: { error: actual },
    complete: false,
    inspectedAt: new Date().toISOString(),
    missing,
    runtimeRevision: "unavailable",
  };
}

function latestEffects(store: Store, operationId: string): RuntimeEffect[] {
  const rows = store.database
    .query<{
      data_json: string;
      effect_key: string;
      kind: string;
      ok: number;
      resource_key: string;
    }, [string]>(
      `SELECT effect_key, kind, resource_key, ok, data_json
       FROM team_operation_effects WHERE operation_id = ? ORDER BY id`,
    )
    .all(operationId);
  const effects = new Map<string, RuntimeEffect>();
  for (const row of rows) {
    effects.set(row.effect_key, {
      data: json<Record<string, unknown>>(row.data_json),
      key: row.effect_key,
      kind: row.kind,
      ok: row.ok === 1,
      resourceKey: row.resource_key,
    });
  }
  return [...effects.values()];
}

function allowed(boundary: TeamControlBoundary, verdict: TeamRow["verdict"]): boolean {
  if (verdict === "OPERABLE") return true;
  if (verdict === "REVIEW_HOLD") {
    return boundary === "work.add" || boundary === "work.start" || boundary === "dispatch.open";
  }
  return false;
}

export class TeamControl {
  constructor(
    private readonly localStore: Store,
    private readonly sessionId: string,
    private readonly runtime: TeamRuntime = new HerdrTeamRuntime(),
  ) {}

  private localBinding(): TeamBinding | null {
    const rows = this.localStore.database
      .query<LocalBindingRow, []>("SELECT * FROM team_local_bindings ORDER BY bound_at DESC")
      .all();
    if (rows.length === 0) return null;
    if (rows.length > 1) {
      throw new Error(`project has ${rows.length} active team bindings; expected one`);
    }
    return fromLocal(rows[0] as LocalBindingRow);
  }

  async check(boundary: TeamControlBoundary): Promise<TeamControlResult> {
    let binding: TeamBinding | null;
    try {
      binding = this.localBinding();
    } catch (error) {
      return {
        allowed: false,
        binding: null,
        bound: true,
        receiptId: null,
        reason: error instanceof Error ? error.message : String(error),
        verdict: "CLOSED",
      };
    }
    if (!binding) {
      return {
        allowed: true,
        binding: null,
        bound: false,
        receiptId: null,
        reason: "project is not bound to a supervised team",
        verdict: null,
      };
    }
    if (canonicalPath(binding.projectStorePath) !== canonicalPath(this.localStore.path)) {
      return {
        allowed: false,
        binding,
        bound: true,
        receiptId: null,
        reason: `project binding store path does not match the current store: bound ${canonicalPath(binding.projectStorePath)}, current ${canonicalPath(this.localStore.path)}`,
        verdict: "CLOSED",
      };
    }
    if (!existsSync(binding.roomStorePath)) {
      return {
        allowed: false,
        binding,
        bound: true,
        receiptId: null,
        reason: `Room ledger unavailable: ${binding.roomStorePath} does not exist`,
        verdict: "CLOSED",
      };
    }
    let roomStore: Store;
    try {
      roomStore = new Store(binding.roomStorePath);
    } catch (error) {
      return {
        allowed: false,
        binding,
        bound: true,
        receiptId: null,
        reason: `Room ledger unavailable: ${error instanceof Error ? error.message : String(error)}`,
        verdict: "CLOSED",
      };
    }
    const operationId = `control-${crypto.randomUUID()}`;
    try {
      const currentIdentity = roomIdentity(roomStore);
      const authority = roomStore.database
        .query<RoomBindingRow, [string]>(
          "SELECT * FROM team_project_bindings WHERE binding_id = ?",
        )
        .get(binding.bindingId);
      if (
        currentIdentity !== binding.roomIdentity ||
        !authority ||
        authority.status !== "ACTIVE" ||
        authority.token !== binding.token ||
        authority.team_id !== binding.teamId ||
        authority.generation !== binding.generation ||
        canonicalPath(authority.project_store_path) !== canonicalPath(binding.projectStorePath)
      ) {
        return {
          allowed: false,
          binding,
          bound: true,
          receiptId: null,
          reason: "project binding does not match the authoritative Room binding",
          verdict: "CLOSED",
        };
      }
      const team = roomStore.database
        .query<TeamRow, [string]>("SELECT * FROM team_lifecycle WHERE team_id = ?")
        .get(binding.teamId);
      if (!team || team.generation !== binding.generation) {
        return {
          allowed: false,
          binding,
          bound: true,
          receiptId: null,
          reason: "Room ledger generation does not match the project binding",
          verdict: "CLOSED",
        };
      }
      const plan = {
        ...json<TeamPlan>(team.plan_json),
        expectedRevision: team.revision,
      };
      const attemptedAt = new Date().toISOString();
      const before = {
        health: team.health,
        review: team.review,
        stage: team.stage,
        verdict: team.verdict,
      };
      roomStore.database
        .query(
          `INSERT INTO team_receipts
            (operation_id, kind, team_id, generation, actor, requested_by, executed_by,
             expected_revision, attempted_at, before_json, desired_json, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED')`,
        )
        .run(
          operationId,
          `team.control.${boundary}`,
          binding.teamId,
          binding.generation,
          this.sessionId,
          this.sessionId,
          this.sessionId,
          team.revision,
          attemptedAt,
          JSON.stringify(before),
          JSON.stringify({ bindingId: binding.bindingId, boundary, plan }),
        );
      let operationEffects: RuntimeEffect[] = [];
      let inspection: TeamInspection;
      try {
        operationEffects = await this.runtime.probeObserver(
          plan,
          operationEffects,
          (effect) => {
            roomStore.database
              .query(
                `INSERT INTO team_operation_effects
                  (operation_id, effect_key, kind, resource_key, ok, data_json, recorded_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)`,
              )
              .run(
                operationId,
                effect.key,
                effect.kind,
                effect.resourceKey,
                effect.ok ? 1 : 0,
                JSON.stringify(effect.data),
                new Date().toISOString(),
              );
          },
        );
        const baseline = json<RuntimeEffect[]>(team.resources_json);
        const combined = [
          ...baseline.filter(
            (effect) => !operationEffects.some((current) => current.key === effect.key),
          ),
          ...operationEffects,
        ];
        inspection = await this.runtime.inspect(plan, combined);
      } catch (error) {
        operationEffects = latestEffects(roomStore, operationId);
        inspection = runtimeFailure(error);
      }
      const refreshed = roomStore.database
        .query<TeamRow, [string]>("SELECT * FROM team_lifecycle WHERE team_id = ?")
        .get(binding.teamId);
      if (!refreshed || refreshed.revision !== team.revision || refreshed.generation !== team.generation) {
        roomStore.database
          .query(
            `UPDATE team_receipts
             SET status = 'FINALIZED', result = 'STALE', completed_at = ?, observed_at = ?,
                 observed_runtime_revision = ?, actual_json = ?, missing_json = ?
             WHERE operation_id = ? AND status = 'ATTEMPTED'`,
          )
          .run(
            new Date().toISOString(),
            inspection.inspectedAt,
            inspection.runtimeRevision,
            JSON.stringify(inspection.actual),
            JSON.stringify(inspection.missing),
            operationId,
          );
        return {
          allowed: false,
          binding,
          bound: true,
          receiptId: operationId,
          reason: "Room ledger changed during the fresh TeamControl inspection",
          verdict: "CLOSED",
        };
      }
      const after = inspection.complete
        ? {
            health: refreshed.health,
            review: refreshed.review,
            stage: refreshed.stage,
          }
        : {
            health: refreshed.stage === "ACTIVE" ? "DEGRADED" as const : refreshed.health,
            review: refreshed.review,
            stage: refreshed.stage,
          };
      const verdict = deriveVerdict(after);
      const completedAt = new Date().toISOString();
      roomStore.database.exec("BEGIN IMMEDIATE");
      try {
        roomStore.database
          .query(
            `UPDATE team_receipts
             SET status = 'FINALIZED', result = ?, completed_at = ?, observed_at = ?,
                 observed_runtime_revision = ?, actual_json = ?, after_json = ?, missing_json = ?
             WHERE operation_id = ? AND status = 'ATTEMPTED'`,
          )
          .run(
            inspection.complete ? verdict : "INSPECTION_FAILED",
            completedAt,
            inspection.inspectedAt,
            inspection.runtimeRevision,
            JSON.stringify(inspection.actual),
            JSON.stringify({ ...after, verdict }),
            JSON.stringify(inspection.missing),
            operationId,
          );
        if (!inspection.complete && refreshed.stage === "ACTIVE" && refreshed.health !== "DEGRADED") {
          roomStore.database
            .query(
              `UPDATE team_lifecycle
               SET health = 'DEGRADED', verdict = 'DRAINING', revision = revision + 1,
                   last_receipt_id = ?, updated_at = ?
               WHERE team_id = ? AND generation = ? AND revision = ?`,
            )
            .run(operationId, completedAt, binding.teamId, binding.generation, refreshed.revision);
        }
        roomStore.database.exec("COMMIT");
      } catch (error) {
        try {
          roomStore.database.exec("ROLLBACK");
        } catch {}
        throw error;
      }
      return {
        allowed: allowed(boundary, verdict),
        binding,
        bound: true,
        receiptId: operationId,
        reason: inspection.complete
          ? `${boundary} denied by team verdict ${verdict}`
          : `${boundary} denied because fresh runtime inspection failed; team is ${verdict}`,
        verdict,
      };
    } catch (error) {
      return {
        allowed: false,
        binding,
        bound: true,
        receiptId: operationId,
        reason: `TeamControl failed closed: ${error instanceof Error ? error.message : String(error)}`,
        verdict: "CLOSED",
      };
    } finally {
      roomStore.close();
    }
  }
}
