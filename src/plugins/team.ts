import { resolve } from "node:path";
import { stat } from "node:fs/promises";
import {
  CliError,
  requiredPosition,
  stringOption,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import {
  buildTeamPlan,
  HerdrTeamRuntime,
  TeamRuntimeError,
  type MissingPostcondition,
  type RuntimeEffect,
  type TeamInspection,
  type TeamPlan,
  type TeamRuntime,
} from "./team-runtime.ts";
import { registerSessionCommand } from "./session-required.ts";

export type TeamStage = "STARTING" | "ACTIVE" | "STOPPING" | "STOPPED";
export type TeamHealth = "READY" | "DEGRADED" | null;
export type TeamReview = "CLEAR" | "REVIEW_REQUIRED";
export type TeamVerdict = "OPERABLE" | "REVIEW_HOLD" | "DRAINING" | "CLOSED";

export interface TeamAxes {
  health: TeamHealth;
  review: TeamReview;
  stage: TeamStage;
}

export interface TeamSnapshot extends TeamAxes {
  createdAt: string;
  generation: number;
  lastReceiptId: string;
  repoPath: string;
  resources: RuntimeEffect[];
  revision: number;
  teamId: string;
  updatedAt: string;
  verdict: TeamVerdict;
  workspaceLabel: string;
}

export interface TeamReceipt {
  actor: string;
  after: (TeamAxes & { verdict: TeamVerdict }) | null;
  attemptedAt: string;
  before: (TeamAxes & { verdict: TeamVerdict }) | null;
  completedAt: string | null;
  executedBy: string;
  expectedRevision: number;
  forced: boolean;
  generation: number;
  kind: string;
  missing: MissingPostcondition[];
  observedAt: string | null;
  observedRuntimeRevision: string | null;
  operationId: string;
  requestedBy: string;
  result: string | null;
  status: "ATTEMPTED" | "FINALIZED";
  teamId: string;
}

interface SnapshotRow {
  created_at: string;
  generation: number;
  health: TeamHealth;
  last_receipt_id: string;
  plan_json: string;
  repo_path: string;
  resources_json: string;
  review: TeamReview;
  revision: number;
  stage: TeamStage;
  team_id: string;
  updated_at: string;
  verdict: TeamVerdict;
  workspace_label: string;
}

interface ReceiptRow {
  actor: string;
  after_json: string | null;
  attempted_at: string;
  before_json: string | null;
  completed_at: string | null;
  desired_json: string;
  executed_by: string;
  expected_revision: number;
  forced: number;
  generation: number;
  kind: string;
  missing_json: string | null;
  observed_at: string | null;
  observed_runtime_revision: string | null;
  operation_id: string;
  requested_by: string;
  result: string | null;
  status: "ATTEMPTED" | "FINALIZED";
  team_id: string;
}

interface EffectRow {
  data_json: string;
  effect_key: string;
  kind: string;
  ok: number;
  resource_key: string;
}

interface DesiredOperation {
  plan: TeamPlan;
}

const rootDescription = "Manage supervised team lifecycle, readiness, review, and shutdown.";

export function deriveTeamVerdict(axes: TeamAxes): TeamVerdict {
  if (axes.stage !== "ACTIVE") return "CLOSED";
  if (axes.health === "DEGRADED") return "DRAINING";
  if (axes.health === "READY" && axes.review === "REVIEW_REQUIRED") return "REVIEW_HOLD";
  return axes.health === "READY" ? "OPERABLE" : "CLOSED";
}

function migrate(context: PluginContext): void {
  context.store.migrate(`
    CREATE TABLE IF NOT EXISTS team_lifecycle (
      team_id TEXT PRIMARY KEY,
      generation INTEGER NOT NULL,
      repo_path TEXT NOT NULL,
      stage TEXT NOT NULL,
      health TEXT,
      review TEXT NOT NULL,
      verdict TEXT NOT NULL,
      revision INTEGER NOT NULL,
      workspace_label TEXT NOT NULL,
      plan_json TEXT NOT NULL,
      resources_json TEXT NOT NULL,
      last_receipt_id TEXT NOT NULL,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS team_receipts (
      operation_id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      actor TEXT NOT NULL,
      requested_by TEXT NOT NULL,
      executed_by TEXT NOT NULL,
      override_reason TEXT,
      expected_revision INTEGER NOT NULL,
      observed_runtime_revision TEXT,
      attempted_at TEXT NOT NULL,
      completed_at TEXT,
      observed_at TEXT,
      before_json TEXT,
      desired_json TEXT NOT NULL,
      actual_json TEXT,
      status TEXT NOT NULL,
      result TEXT,
      after_json TEXT,
      missing_json TEXT,
      forced INTEGER NOT NULL DEFAULT 0,
      error_json TEXT
    );
    CREATE INDEX IF NOT EXISTS team_receipts_team_generation
      ON team_receipts(team_id, generation, attempted_at);
    CREATE TABLE IF NOT EXISTS team_operation_effects (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      operation_id TEXT NOT NULL,
      effect_key TEXT NOT NULL,
      kind TEXT NOT NULL,
      resource_key TEXT NOT NULL,
      ok INTEGER NOT NULL,
      data_json TEXT NOT NULL,
      recorded_at TEXT NOT NULL,
      FOREIGN KEY(operation_id) REFERENCES team_receipts(operation_id)
    );
    CREATE INDEX IF NOT EXISTS team_operation_effects_operation
      ON team_operation_effects(operation_id, id);
  `);
}

function normalizeTeamId(raw: string): string {
  const normalized = raw.toLowerCase().replace(/^team-/, "");
  if (!/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(normalized)) {
    throw new CliError(
      "INVALID_TEAM_ID",
      "team id must use 1-64 lowercase letters, numbers, or internal hyphens",
      { teamId: raw },
    );
  }
  return normalized;
}

function requiredOperation(invocation: CliInvocation): string {
  const operationId = stringOption(invocation, "operation");
  if (!operationId) throw new CliError("MISSING_ARGUMENT", "missing --operation");
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(operationId)) {
    throw new CliError("INVALID_OPERATION_ID", "operation id contains unsupported characters", {
      operationId,
    });
  }
  return operationId;
}

function nonNegativeIntegerOption(
  invocation: CliInvocation,
  name: string,
  fallback: number,
  maximum: number,
): number {
  const raw = stringOption(invocation, name);
  if (raw === undefined) return fallback;
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new CliError(
      "INVALID_ARGUMENT",
      `--${name} must be an integer from 0 through ${maximum}`,
      { name, value: raw },
    );
  }
  return value;
}

function json<T>(value: string | null, fallback: T): T {
  return value === null ? fallback : JSON.parse(value) as T;
}

function snapshotFromRow(row: SnapshotRow): TeamSnapshot {
  return {
    createdAt: row.created_at,
    generation: row.generation,
    health: row.health,
    lastReceiptId: row.last_receipt_id,
    repoPath: row.repo_path,
    resources: json<RuntimeEffect[]>(row.resources_json, []),
    review: row.review,
    revision: row.revision,
    stage: row.stage,
    teamId: row.team_id,
    updatedAt: row.updated_at,
    verdict: row.verdict,
    workspaceLabel: row.workspace_label,
  };
}

function receiptFromRow(row: ReceiptRow): TeamReceipt {
  return {
    actor: row.actor,
    after: json<TeamReceipt["after"]>(row.after_json, null),
    attemptedAt: row.attempted_at,
    before: json<TeamReceipt["before"]>(row.before_json, null),
    completedAt: row.completed_at,
    executedBy: row.executed_by,
    expectedRevision: row.expected_revision,
    forced: row.forced === 1,
    generation: row.generation,
    kind: row.kind,
    missing: json<MissingPostcondition[]>(row.missing_json, []),
    observedAt: row.observed_at,
    observedRuntimeRevision: row.observed_runtime_revision,
    operationId: row.operation_id,
    requestedBy: row.requested_by,
    result: row.result,
    status: row.status,
    teamId: row.team_id,
  };
}

function findSnapshot(context: PluginContext, teamId: string): TeamSnapshot | null {
  const row = context.store.database
    .query<SnapshotRow, [string]>("SELECT * FROM team_lifecycle WHERE team_id = ?")
    .get(teamId);
  return row ? snapshotFromRow(row) : null;
}

function requireSnapshot(context: PluginContext, teamId: string): TeamSnapshot {
  const snapshot = findSnapshot(context, teamId);
  if (!snapshot) throw new CliError("NOT_FOUND", `team not found: ${teamId}`, { teamId });
  return snapshot;
}

function findReceipt(context: PluginContext, operationId: string): ReceiptRow | null {
  return context.store.database
    .query<ReceiptRow, [string]>("SELECT * FROM team_receipts WHERE operation_id = ?")
    .get(operationId) ?? null;
}

function latestEffects(context: PluginContext, operationId: string): RuntimeEffect[] {
  const rows = context.store.database
    .query<EffectRow, [string]>(
      `SELECT effect_key, kind, resource_key, ok, data_json
       FROM team_operation_effects
       WHERE operation_id = ?
       ORDER BY id`,
    )
    .all(operationId);
  const effects = new Map<string, RuntimeEffect>();
  for (const row of rows) {
    effects.set(row.effect_key, {
      data: json<Record<string, unknown>>(row.data_json, {}),
      key: row.effect_key,
      kind: row.kind,
      ok: row.ok === 1,
      resourceKey: row.resource_key,
    });
  }
  return [...effects.values()];
}

function recordEffect(
  context: PluginContext,
  operationId: string,
  effect: RuntimeEffect,
): void {
  context.store.database
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
}

function baseAxes(snapshot: TeamSnapshot | null): TeamReceipt["before"] {
  if (!snapshot) return null;
  return {
    health: snapshot.health,
    review: snapshot.review,
    stage: snapshot.stage,
    verdict: snapshot.verdict,
  };
}

function validateReceiptIdentity(
  row: ReceiptRow,
  kind: string,
  teamId: string,
): void {
  if (row.kind !== kind || row.team_id !== teamId) {
    throw new CliError(
      "OPERATION_CONFLICT",
      `${row.operation_id} already belongs to ${row.kind} for ${row.team_id}`,
      { kind: row.kind, operationId: row.operation_id, teamId: row.team_id },
    );
  }
}

function insertAttempt(
  context: PluginContext,
  input: {
    desired: DesiredOperation;
    generation: number;
    kind: string;
    operationId: string;
    requestedBy: string;
    snapshot: TeamSnapshot | null;
    teamId: string;
  },
): ReceiptRow {
  const session = context.sessions.current();
  const attemptedAt = new Date().toISOString();
  context.store.database
    .query(
      `INSERT INTO team_receipts
        (operation_id, kind, team_id, generation, actor, requested_by, executed_by,
         expected_revision, attempted_at, before_json, desired_json, status)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED')`,
    )
    .run(
      input.operationId,
      input.kind,
      input.teamId,
      input.generation,
      session.id,
      input.requestedBy,
      session.id,
      input.desired.plan.expectedRevision,
      attemptedAt,
      JSON.stringify(baseAxes(input.snapshot)),
      JSON.stringify(input.desired),
    );
  context.sessions.record(input.kind);
  return findReceipt(context, input.operationId) as ReceiptRow;
}

function runtimeFailureInspection(error: unknown): TeamInspection {
  const details = error instanceof TeamRuntimeError
    ? { command: error.command, message: error.message, stderr: error.stderr ?? null }
    : { message: error instanceof Error ? error.message : String(error) };
  const missing: MissingPostcondition[] = [{
    actual: details,
    code: "runtime.unavailable",
    expected: "successful TeamRuntime inspection",
    resource: "runtime",
  }];
  return {
    actual: { error: details },
    complete: false,
    inspectedAt: new Date().toISOString(),
    missing,
    runtimeRevision: "unavailable",
  };
}

async function inspectUntil(
  runtime: TeamRuntime,
  plan: TeamPlan,
  effects: readonly RuntimeEffect[],
  waitMs: number,
): Promise<TeamInspection> {
  const deadline = Date.now() + waitMs;
  while (true) {
    const inspection = await runtime.inspect(plan, effects);
    if (inspection.complete || Date.now() >= deadline) return inspection;
    await Bun.sleep(Math.min(100, Math.max(1, deadline - Date.now())));
  }
}

function finalizeReceiptAndSnapshot(
  context: PluginContext,
  input: {
    axes: TeamAxes;
    effects: RuntimeEffect[];
    inspection: TeamInspection;
    operationId: string;
    plan: TeamPlan;
    previous: TeamSnapshot | null;
    result: string;
  },
): { receipt: TeamReceipt; team: TeamSnapshot } {
  const now = new Date().toISOString();
  const after = { ...input.axes, verdict: deriveTeamVerdict(input.axes) };
  const revision = input.plan.expectedRevision + 1;
  context.store.database.exec("BEGIN IMMEDIATE");
  try {
    const current = findSnapshot(context, input.plan.teamId);
    const currentRevision = current?.revision ?? 0;
    if (currentRevision !== input.plan.expectedRevision) {
      throw new CliError(
        "STALE_REVISION",
        `team ${input.plan.teamId} revision changed from ${input.plan.expectedRevision} to ${currentRevision}`,
        { actualRevision: currentRevision, expectedRevision: input.plan.expectedRevision },
      );
    }
    const finalized = context.store.database
      .query(
        `UPDATE team_receipts
         SET observed_runtime_revision = ?, completed_at = ?, observed_at = ?,
             actual_json = ?, status = 'FINALIZED', result = ?, after_json = ?, missing_json = ?
         WHERE operation_id = ? AND status = 'ATTEMPTED'`,
      )
      .run(
        input.inspection.runtimeRevision,
        now,
        input.inspection.inspectedAt,
        JSON.stringify(input.inspection.actual),
        input.result,
        JSON.stringify(after),
        JSON.stringify(input.inspection.missing),
        input.operationId,
      );
    if (finalized.changes !== 1) {
      throw new CliError("OPERATION_FINALIZED", `${input.operationId} was already finalized`);
    }
    context.store.database
      .query(
        `INSERT INTO team_lifecycle
          (team_id, generation, repo_path, stage, health, review, verdict, revision,
           workspace_label, plan_json, resources_json, last_receipt_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(team_id) DO UPDATE SET
           generation = excluded.generation,
           repo_path = excluded.repo_path,
           stage = excluded.stage,
           health = excluded.health,
           review = excluded.review,
           verdict = excluded.verdict,
           revision = excluded.revision,
           workspace_label = excluded.workspace_label,
           plan_json = excluded.plan_json,
           resources_json = excluded.resources_json,
           last_receipt_id = excluded.last_receipt_id,
           updated_at = excluded.updated_at`,
      )
      .run(
        input.plan.teamId,
        input.plan.generation,
        input.plan.repoPath,
        input.axes.stage,
        input.axes.health,
        input.axes.review,
        after.verdict,
        revision,
        input.plan.workspaceLabel,
        JSON.stringify(input.plan),
        JSON.stringify(input.effects),
        input.operationId,
        input.previous?.createdAt ?? now,
        now,
      );
    context.store.database.exec("COMMIT");
  } catch (error) {
    try {
      context.store.database.exec("ROLLBACK");
    } catch {}
    throw error;
  }
  const receiptRow = findReceipt(context, input.operationId);
  if (!receiptRow) throw new Error(`finalized receipt disappeared: ${input.operationId}`);
  return {
    receipt: receiptFromRow(receiptRow),
    team: requireSnapshot(context, input.plan.teamId),
  };
}

function operationResult(
  outcome: { receipt: TeamReceipt; team: TeamSnapshot },
  incompleteCode: "TEAM_STARTING" | "TEAM_NOT_READY",
): CliResult {
  if (outcome.team.verdict !== "OPERABLE") {
    throw new CliError(
      incompleteCode,
      `${outcome.team.teamId} is ${outcome.team.stage}/${outcome.team.health ?? "UNPROVEN"}; missing ${outcome.receipt.missing.length} postconditions`,
      { receipt: outcome.receipt, team: outcome.team },
    );
  }
  return {
    data: { receipt: outcome.receipt, team: outcome.team },
    text: `${outcome.team.teamId} OPERABLE generation ${outcome.team.generation} revision ${outcome.team.revision}`,
  };
}

function replayResult(
  context: PluginContext,
  row: ReceiptRow,
  incompleteCode: "TEAM_STARTING" | "TEAM_NOT_READY",
): CliResult {
  const receipt = receiptFromRow(row);
  const team = requireSnapshot(context, row.team_id);
  return operationResult({ receipt, team }, incompleteCode);
}

async function openTeam(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existingReceipt = findReceipt(context, operationId);
  if (existingReceipt) {
    validateReceiptIdentity(existingReceipt, "team.open", teamId);
    if (existingReceipt.status === "FINALIZED") {
      return replayResult(context, existingReceipt, "TEAM_STARTING");
    }
  }

  let receiptRow = existingReceipt;
  let desired: DesiredOperation;
  let previous: TeamSnapshot | null;
  if (receiptRow) {
    desired = json<DesiredOperation>(receiptRow.desired_json, {} as DesiredOperation);
    previous = findSnapshot(context, teamId);
  } else {
    previous = findSnapshot(context, teamId);
    if (previous && previous.stage !== "STOPPED") {
      throw new CliError(
        "TEAM_ALREADY_OPEN",
        `${teamId} is ${previous.stage}; use team await-ready, health, reconcile, or stop`,
        { stage: previous.stage, teamId },
      );
    }
    const repoOption = stringOption(invocation, "repo");
    if (!repoOption) throw new CliError("MISSING_ARGUMENT", "missing --repo");
    const repoPath = resolve(repoOption);
    let repoExists = false;
    try {
      repoExists = (await stat(repoPath)).isDirectory();
    } catch {}
    if (!repoExists) {
      throw new CliError("NOT_FOUND", `repository path not found: ${repoPath}`, { repoPath });
    }
    const expectedRevision = nonNegativeIntegerOption(
      invocation,
      "expected-revision",
      previous?.revision ?? 0,
      Number.MAX_SAFE_INTEGER,
    );
    if (expectedRevision !== (previous?.revision ?? 0)) {
      throw new CliError("STALE_REVISION", `expected revision ${expectedRevision} does not match current team revision`, {
        actualRevision: previous?.revision ?? 0,
        expectedRevision,
      });
    }
    const generation = previous ? previous.generation + 1 : 1;
    desired = {
      plan: buildTeamPlan({ expectedRevision, generation, repoPath, teamId }),
    };
    receiptRow = insertAttempt(context, {
      desired,
      generation,
      kind: "team.open",
      operationId,
      requestedBy: stringOption(invocation, "requested-by") ?? context.sessions.current().id,
      snapshot: previous,
      teamId,
    });
  }

  let effects = latestEffects(context, operationId);
  let inspection: TeamInspection;
  try {
    effects = await runtime.ensure(
      desired.plan,
      effects,
      (effect) => recordEffect(context, operationId, effect),
    );
    inspection = await inspectUntil(
      runtime,
      desired.plan,
      effects,
      nonNegativeIntegerOption(invocation, "wait-ms", 5_000, 600_000),
    );
  } catch (error) {
    effects = latestEffects(context, operationId);
    inspection = runtimeFailureInspection(error);
  }
  const axes: TeamAxes = inspection.complete
    ? { health: "READY", review: "CLEAR", stage: "ACTIVE" }
    : { health: null, review: "CLEAR", stage: "STARTING" };
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes,
    effects,
    inspection,
    operationId,
    plan: desired.plan,
    previous,
    result: inspection.complete ? "OPERABLE" : "STARTING",
  });
  context.log.append({
    entityId: teamId,
    entityType: "team",
    payload: {
      generation: desired.plan.generation,
      missing: inspection.missing,
      operationId,
      result: outcome.receipt.result,
      revision: outcome.team.revision,
    },
    sessionId: context.sessions.current().id,
    type: "team.open",
  });
  return operationResult(outcome, "TEAM_STARTING");
}

async function inspectHealth(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
  kind: "team.health" | "team.await-ready",
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existingReceipt = findReceipt(context, operationId);
  if (existingReceipt) {
    validateReceiptIdentity(existingReceipt, kind, teamId);
    if (existingReceipt.status === "FINALIZED") {
      return replayResult(context, existingReceipt, "TEAM_NOT_READY");
    }
  }
  const previous = requireSnapshot(context, teamId);
  if (previous.stage === "STOPPING" || previous.stage === "STOPPED") {
    throw new CliError("INVALID_STATE", `${teamId} is ${previous.stage}`, {
      stage: previous.stage,
      teamId,
    });
  }
  let receiptRow = existingReceipt;
  let desired: DesiredOperation;
  if (receiptRow) {
    desired = json<DesiredOperation>(receiptRow.desired_json, {} as DesiredOperation);
  } else {
    const expectedRevision = nonNegativeIntegerOption(
      invocation,
      "expected-revision",
      previous.revision,
      Number.MAX_SAFE_INTEGER,
    );
    if (expectedRevision !== previous.revision) {
      throw new CliError("STALE_REVISION", `expected revision ${expectedRevision} does not match ${previous.revision}`, {
        actualRevision: previous.revision,
        expectedRevision,
      });
    }
    const storedPlan = context.store.database
      .query<{ plan_json: string }, [string]>("SELECT plan_json FROM team_lifecycle WHERE team_id = ?")
      .get(teamId);
    if (!storedPlan) throw new CliError("NOT_FOUND", `team not found: ${teamId}`);
    desired = {
      plan: {
        ...json<TeamPlan>(storedPlan.plan_json, {} as TeamPlan),
        expectedRevision,
      },
    };
    receiptRow = insertAttempt(context, {
      desired,
      generation: previous.generation,
      kind,
      operationId,
      requestedBy: stringOption(invocation, "requested-by") ?? context.sessions.current().id,
      snapshot: previous,
      teamId,
    });
  }

  let currentEffects = latestEffects(context, operationId);
  let inspection: TeamInspection;
  try {
    currentEffects = await runtime.probeObserver(
      desired.plan,
      currentEffects,
      (effect) => recordEffect(context, operationId, effect),
    );
    const combinedEffects = [
      ...previous.resources.filter(
        (effect) => !currentEffects.some((current) => current.key === effect.key),
      ),
      ...currentEffects,
    ];
    inspection = await inspectUntil(
      runtime,
      desired.plan,
      combinedEffects,
      kind === "team.await-ready"
        ? nonNegativeIntegerOption(invocation, "wait-ms", 5_000, 600_000)
        : 0,
    );
    currentEffects = combinedEffects;
  } catch (error) {
    currentEffects = [
      ...previous.resources,
      ...latestEffects(context, operationId),
    ];
    inspection = runtimeFailureInspection(error);
  }
  const axes: TeamAxes = inspection.complete
    ? { health: "READY", review: previous.review, stage: "ACTIVE" }
    : previous.stage === "STARTING"
      ? { health: null, review: previous.review, stage: "STARTING" }
      : { health: "DEGRADED", review: previous.review, stage: "ACTIVE" };
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes,
    effects: currentEffects,
    inspection,
    operationId,
    plan: desired.plan,
    previous,
    result: inspection.complete ? deriveTeamVerdict(axes) : axes.stage === "STARTING" ? "STARTING" : "DEGRADED",
  });
  context.log.append({
    entityId: teamId,
    entityType: "team",
    payload: {
      generation: previous.generation,
      kind,
      missing: inspection.missing,
      operationId,
      result: outcome.receipt.result,
      revision: outcome.team.revision,
    },
    sessionId: context.sessions.current().id,
    type: kind,
  });
  if (kind === "team.await-ready") return operationResult(outcome, "TEAM_NOT_READY");
  return {
    data: { receipt: outcome.receipt, team: outcome.team },
    text: `${teamId} ${outcome.team.stage}/${outcome.team.health ?? "UNPROVEN"} ${outcome.team.verdict}`,
  };
}

const commonMutationFlags = {
  "--expected-revision": {
    description: "Require the current Room-ledger revision before effects.",
    value: true,
  },
  "--operation": {
    description: "Use a stable idempotency key for this lifecycle operation.",
    value: true,
  },
  "--requested-by": {
    description: "Record the authorizing actor separately from the executing session.",
    value: true,
  },
};

export const teamPlugin: BuiltInPlugin = {
  name: "team",
  apply(context) {
    migrate(context);
    const runtime: TeamRuntime = new HerdrTeamRuntime();
    context.effect(() => context.provide("teamRuntime", runtime));
    context.effect(() =>
      registerSessionCommand(
        context,
        "team open",
        (invocation) => openTeam(context, runtime, invocation),
        {
          description: "Create or adopt a team and prove bounded readiness.",
          flags: {
            ...commonMutationFlags,
            "--repo": { description: "Set the repository cwd owned by the team.", value: true },
            "--wait-ms": { description: "Bound the foreground readiness wait.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      context.cli.register(
        "team status",
        (invocation): CliResult => {
          const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
          const team = requireSnapshot(context, teamId);
          return {
            data: { team },
            text: `${teamId} ${team.stage}/${team.health ?? "UNPROVEN"}/${team.review} ${team.verdict} generation ${team.generation} revision ${team.revision}`,
          };
        },
        {
          description: "Read the current Room-ledger team snapshot.",
          json: true,
          mutates: false,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team health",
        (invocation) => inspectHealth(context, runtime, invocation, "team.health"),
        {
          description: "Inspect runtime health and record a receipt.",
          flags: commonMutationFlags,
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team await-ready",
        (invocation) => inspectHealth(context, runtime, invocation, "team.await-ready"),
        {
          description: "Wait in the foreground for bounded team readiness.",
          flags: {
            ...commonMutationFlags,
            "--wait-ms": { description: "Bound the foreground readiness wait.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    for (const [command, description] of [
      ["team reconcile", "Repair explicitly selected team resources and re-prove readiness."],
      ["team review", "Raise, inspect, resolve, or escalate team review."],
      ["team advise", "Run one bounded Advisor consultation."],
      ["team stop", "Drain and stop one team generation."],
    ] as const) {
      context.effect(() =>
        registerSessionCommand(
          context,
          command,
          () => {
            throw new CliError(
              "TEAM_OPERATION_UNAVAILABLE",
              `${command} is not available in the readiness-core slice`,
            );
          },
          { description, rootDescription },
        ),
      );
    }
  },
};
