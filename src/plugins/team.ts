import { resolve } from "node:path";
import { stat } from "node:fs/promises";
import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { Store } from "../kernel/store.ts";
import {
  buildTeamPlan,
  HerdrTeamRuntime,
  TeamRuntimeError,
  type AdvisorConsultationRequest,
  type MissingPostcondition,
  type RuntimeEffect,
  type TeamInspection,
  type TeamPlan,
  type TeamRepairResource,
  type TeamRuntime,
} from "./team-runtime.ts";
import { registerSessionCommand } from "./session-required.ts";
import {
  bindProject,
  migrateTeamControlTables,
  TeamControl,
  type TeamControlBoundary,
} from "./team-control.ts";
import {
  buildEvidencePacket,
  spotCheckRule,
  triggerRule,
  type BoundedEvidencePacket,
  type TriggerRule,
} from "./team-observer.ts";

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

export interface TeamOverrideEvidence {
  basis: "supervisor-unreachable" | "owner-intervention";
  declared: string;
  inspectedAt: string | null;
  missing: MissingPostcondition[];
  runtimeRevision: string | null;
}

export interface TeamReceipt {
  actor: string;
  actual: Record<string, unknown> | null;
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
  overrideEvidence: TeamOverrideEvidence | null;
  overrideReason: string | null;
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
  actual_json: string | null;
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
  override_evidence_json: string | null;
  override_reason: string | null;
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
  force?: {
    evidence: string;
    reason: string;
  };
  plan: TeamPlan;
}

interface TeamAuthorityOverride {
  evidence: TeamOverrideEvidence;
  reason: string;
}

interface ReviewPacketRow {
  actor: string;
  authority_ref: string | null;
  capability: string;
  created_at: string;
  decision_ref: string | null;
  dedupe_key: string;
  delivered_at: string | null;
  evidence: string;
  excerpt: string;
  finding: string | null;
  generation: number;
  health_receipt_id: string;
  id: string;
  operation_id: string;
  resolved_at: string | null;
  rule_id: string;
  rule_version: number;
  status: "ATTEMPTED" | "DELIVERED" | "DELIVERY_FAILED" | "VERDICTED" | "RESOLVED";
  stop_ref: string | null;
  team_id: string;
  truncated: number;
  verdict: string | null;
  work_ref: string | null;
}

interface ReviewPacket extends BoundedEvidencePacket {
  capability: string;
  createdAt: string;
  deliveredAt: string | null;
  finding: string | null;
  id: string;
  operationId: string;
  resolvedAt: string | null;
  status: ReviewPacketRow["status"];
  verdict: string | null;
}

interface AdvisorConsultationRow {
  completed_at: string | null;
  context_json: string;
  created_at: string;
  decision_ref: string;
  error: string | null;
  generation: number;
  operation_id: string;
  question: string;
  recommendation: string | null;
  requested_by: string;
  status: "ATTEMPTED" | "COMPLETED" | "FAILED";
  stop_condition: string;
  team_id: string;
}

interface AdvisorConsultation {
  completedAt: string | null;
  contextRefs: string[];
  createdAt: string;
  decisionRef: string;
  error: string | null;
  generation: number;
  operationId: string;
  question: string;
  recommendation: string | null;
  requestedBy: string;
  status: AdvisorConsultationRow["status"];
  stopCondition: string;
  teamId: string;
}

interface BindingReceiptRow {
  binding_json: string | null;
  completed_at: string | null;
  executed_by: string;
  generation: number;
  operation_id: string;
  project_root: string;
  requested_by: string;
  status: "ATTEMPTED" | "FINALIZED" | "FAILED";
  team_id: string;
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
      override_evidence_json TEXT,
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
    CREATE TABLE IF NOT EXISTS team_review_packets (
      id TEXT PRIMARY KEY,
      operation_id TEXT NOT NULL,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      rule_id TEXT NOT NULL,
      rule_version INTEGER NOT NULL,
      actor TEXT NOT NULL,
      work_ref TEXT,
      decision_ref TEXT,
      authority_ref TEXT,
      stop_ref TEXT,
      evidence TEXT NOT NULL,
      excerpt TEXT NOT NULL,
      truncated INTEGER NOT NULL,
      health_receipt_id TEXT NOT NULL,
      dedupe_key TEXT NOT NULL,
      capability TEXT NOT NULL,
      status TEXT NOT NULL,
      created_at TEXT NOT NULL,
      delivered_at TEXT,
      verdict TEXT,
      finding TEXT,
      resolved_at TEXT,
      UNIQUE(team_id, generation, dedupe_key),
      FOREIGN KEY(operation_id) REFERENCES team_receipts(operation_id)
    );
    CREATE INDEX IF NOT EXISTS team_review_packets_team_status
      ON team_review_packets(team_id, generation, status, created_at);
    CREATE TABLE IF NOT EXISTS team_advisor_consultations (
      operation_id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      requested_by TEXT NOT NULL,
      decision_ref TEXT NOT NULL,
      question TEXT NOT NULL,
      context_json TEXT NOT NULL,
      stop_condition TEXT NOT NULL,
      status TEXT NOT NULL,
      recommendation TEXT,
      error TEXT,
      created_at TEXT NOT NULL,
      completed_at TEXT,
      FOREIGN KEY(operation_id) REFERENCES team_receipts(operation_id)
    );
    CREATE TABLE IF NOT EXISTS team_binding_receipts (
      operation_id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      project_root TEXT NOT NULL,
      requested_by TEXT NOT NULL,
      executed_by TEXT NOT NULL,
      status TEXT NOT NULL,
      binding_json TEXT,
      error TEXT,
      attempted_at TEXT NOT NULL,
      completed_at TEXT
    );
  `);
  context.store.ensureColumn(
    "team_receipts",
    "override_reason",
    "ALTER TABLE team_receipts ADD COLUMN override_reason TEXT",
  );
  context.store.ensureColumn(
    "team_receipts",
    "override_evidence_json",
    "ALTER TABLE team_receipts ADD COLUMN override_evidence_json TEXT",
  );
  migrateTeamControlTables(context.store, process.cwd());
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

function requiredStringOption(invocation: CliInvocation, name: string): string {
  const value = stringOption(invocation, name)?.trim();
  if (!value) throw new CliError("MISSING_ARGUMENT", `missing --${name}`);
  return value;
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
    actual: json<Record<string, unknown> | null>(row.actual_json, null),
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
    overrideEvidence: json<TeamOverrideEvidence | null>(row.override_evidence_json ?? null, null),
    overrideReason: row.override_reason ?? null,
    requestedBy: row.requested_by,
    result: row.result,
    status: row.status,
    teamId: row.team_id,
  };
}

function packetFromRow(row: ReviewPacketRow): ReviewPacket {
  return {
    actor: row.actor,
    authorityRef: row.authority_ref,
    capability: row.capability,
    createdAt: row.created_at,
    decisionRef: row.decision_ref,
    dedupeKey: row.dedupe_key,
    deliveredAt: row.delivered_at,
    evidence: row.evidence,
    excerpt: row.excerpt,
    finding: row.finding,
    generation: row.generation,
    healthReceiptId: row.health_receipt_id,
    id: row.id,
    operationId: row.operation_id,
    resolvedAt: row.resolved_at,
    ruleId: row.rule_id,
    ruleVersion: row.rule_version,
    status: row.status,
    stopRef: row.stop_ref,
    teamId: row.team_id,
    truncated: row.truncated === 1,
    verdict: row.verdict,
    workRef: row.work_ref,
  };
}

function findPacket(context: PluginContext, packetId: string): ReviewPacketRow | null {
  return context.store.database
    .query<ReviewPacketRow, [string]>("SELECT * FROM team_review_packets WHERE id = ?")
    .get(packetId) ?? null;
}

function consultationFromRow(row: AdvisorConsultationRow): AdvisorConsultation {
  return {
    completedAt: row.completed_at,
    contextRefs: json<string[]>(row.context_json, []),
    createdAt: row.created_at,
    decisionRef: row.decision_ref,
    error: row.error,
    generation: row.generation,
    operationId: row.operation_id,
    question: row.question,
    recommendation: row.recommendation,
    requestedBy: row.requested_by,
    status: row.status,
    stopCondition: row.stop_condition,
    teamId: row.team_id,
  };
}

function findConsultation(
  context: PluginContext,
  operationId: string,
): AdvisorConsultationRow | null {
  return context.store.database
    .query<AdvisorConsultationRow, [string]>(
      "SELECT * FROM team_advisor_consultations WHERE operation_id = ?",
    )
    .get(operationId) ?? null;
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
    override?: TeamAuthorityOverride | null;
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
         override_reason, override_evidence_json, expected_revision, attempted_at,
         before_json, desired_json, status)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED')`,
    )
    .run(
      input.operationId,
      input.kind,
      input.teamId,
      input.generation,
      session.id,
      input.requestedBy,
      session.id,
      input.override?.reason ?? null,
      input.override ? JSON.stringify(input.override.evidence) : null,
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
    forced?: boolean;
    inspection: TeamInspection;
    operationId: string;
    plan: TeamPlan;
    previous: TeamSnapshot | null;
    result: string;
    transactionMutation?: () => void;
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
             actual_json = ?, status = 'FINALIZED', result = ?, after_json = ?, missing_json = ?,
             forced = ?
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
        input.forced ? 1 : 0,
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
    input.transactionMutation?.();
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
  try {
    bindProject({
      generation: outcome.team.generation,
      projectRoot: outcome.team.repoPath,
      roomRoot: process.cwd(),
      roomStore: context.store,
      teamId,
    });
  } catch (error) {
    throw new CliError(
      "TEAM_BINDING_FAILED",
      `team ${teamId} opened but its project binding failed: ${error instanceof Error ? error.message : String(error)}`,
      { receipt: outcome.receipt, team: outcome.team },
    );
  }
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

function planForSnapshot(context: PluginContext, snapshot: TeamSnapshot): TeamPlan {
  const stored = context.store.database
    .query<{ plan_json: string }, [string]>(
      "SELECT plan_json FROM team_lifecycle WHERE team_id = ?",
    )
    .get(snapshot.teamId);
  if (!stored) throw new CliError("NOT_FOUND", `team not found: ${snapshot.teamId}`);
  return {
    ...json<TeamPlan>(stored.plan_json, {} as TeamPlan),
    expectedRevision: snapshot.revision,
  };
}

function receiptInspection(input: {
  actual: Record<string, unknown>;
  complete?: boolean;
  missing?: MissingPostcondition[];
  revision: string;
}): TeamInspection {
  return {
    actual: input.actual,
    complete: input.complete ?? true,
    inspectedAt: new Date().toISOString(),
    missing: input.missing ?? [],
    runtimeRevision: input.revision,
  };
}

function latestHealthReceiptId(
  context: PluginContext,
  teamId: string,
  generation: number,
): string {
  return context.store.database
    .query<{ operation_id: string }, [string, number]>(
      `SELECT operation_id
       FROM team_receipts
       WHERE team_id = ? AND generation = ? AND status = 'FINALIZED'
         AND kind IN ('team.open', 'team.health', 'team.await-ready', 'team.reconcile')
       ORDER BY completed_at DESC
       LIMIT 1`,
    )
    .get(teamId, generation)?.operation_id ?? "unproven";
}

function packetPrompt(packet: ReviewPacket): string {
  return [
    `[observer-packet ${packet.id}]`,
    `capability=${packet.capability}`,
    `team=${packet.teamId}`,
    `generation=${packet.generation}`,
    `rule=${packet.ruleId}@${packet.ruleVersion}`,
    `actor=${packet.actor}`,
    `work=${packet.workRef ?? "none"}`,
    `decision=${packet.decisionRef ?? "none"}`,
    `authority=${packet.authorityRef ?? "none"}`,
    `stop=${packet.stopRef ?? "none"}`,
    `healthReceipt=${packet.healthReceiptId}`,
    `evidence=${JSON.stringify(packet.evidence)}`,
    `excerpt=${JSON.stringify(packet.excerpt)}`,
    "Submit at most one packet-bound verdict through team review raise; do not inspect other records or mutate runtime.",
  ].join("\n");
}

async function triggerReviewPacket(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
  selectedRule?: TriggerRule,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const snapshot = requireSnapshot(context, teamId);
  if (snapshot.stage !== "ACTIVE") {
    throw new CliError("INVALID_STATE", `${teamId} is ${snapshot.stage}; semantic review requires ACTIVE`, {
      stage: snapshot.stage,
      teamId,
    });
  }
  if (selectedRule) {
    assertTeamSupervisor(teamId, requiredStringOption(invocation, "requested-by"));
  }
  const rule = selectedRule ?? triggerRule(requiredStringOption(invocation, "rule"));
  if (!rule || rule.consequence !== "REVIEW") {
    throw new CliError("UNKNOWN_TRIGGER_RULE", "review trigger requires a versioned semantic rule", {
      rule: stringOption(invocation, "rule") ?? null,
    });
  }
  const occurrences = nonNegativeIntegerOption(invocation, "occurrences", 1, 1_000_000);
  if (occurrences < rule.minimumOccurrences) {
    throw new CliError(
      "TRIGGER_THRESHOLD_NOT_MET",
      `${rule.id}@${rule.version} requires ${rule.minimumOccurrences} occurrences; observed ${occurrences}`,
      { minimumOccurrences: rule.minimumOccurrences, observedOccurrences: occurrences, ruleId: rule.id },
    );
  }
  const operationId = requiredOperation(invocation);
  const evidence = requiredStringOption(invocation, selectedRule ? "question" : "evidence");
  const bounded = buildEvidencePacket({
    actor: selectedRule ? requiredStringOption(invocation, "requested-by") : requiredStringOption(invocation, "actor"),
    authorityRef: stringOption(invocation, "authority"),
    decisionRef: stringOption(invocation, "decision"),
    evidence,
    excerpt: stringOption(invocation, "excerpt") ?? stringOption(invocation, "window"),
    generation: snapshot.generation,
    healthReceiptId: latestHealthReceiptId(context, teamId, snapshot.generation),
    rule,
    stopRef: stringOption(invocation, "stop") ?? "one packet-bound verdict",
    teamId,
    workRef: stringOption(invocation, "work"),
  });
  const duplicate = context.store.database
    .query<ReviewPacketRow, [string, number, string]>(
      "SELECT * FROM team_review_packets WHERE team_id = ? AND generation = ? AND dedupe_key = ?",
    )
    .get(teamId, snapshot.generation, bounded.dedupeKey);
  if (duplicate) {
    return {
      data: { deduped: true, packet: packetFromRow(duplicate), team: snapshot },
      text: `${duplicate.id} deduped: ${rule.id}@${rule.version}`,
    };
  }
  const existingReceipt = findReceipt(context, operationId);
  if (existingReceipt) {
    validateReceiptIdentity(existingReceipt, "team.review.trigger", teamId);
    const existingPacket = context.store.database
      .query<ReviewPacketRow, [string]>("SELECT * FROM team_review_packets WHERE operation_id = ?")
      .get(operationId);
    if (existingReceipt.status === "FINALIZED" && existingPacket) {
      return {
        data: { deduped: false, packet: packetFromRow(existingPacket), receipt: receiptFromRow(existingReceipt), team: snapshot },
        text: `${existingPacket.id} ${existingPacket.status}`,
      };
    }
  }
  const plan = planForSnapshot(context, snapshot);
  if (!existingReceipt) {
    insertAttempt(context, {
      desired: { plan },
      generation: snapshot.generation,
      kind: "team.review.trigger",
      operationId,
      requestedBy: selectedRule
        ? requiredStringOption(invocation, "requested-by")
        : context.sessions.current().id,
      snapshot,
      teamId,
    });
  }
  let row = context.store.database
    .query<ReviewPacketRow, [string]>("SELECT * FROM team_review_packets WHERE operation_id = ?")
    .get(operationId);
  if (!row) {
    const createdAt = new Date().toISOString();
    const id = `p-${crypto.randomUUID()}`;
    const capability = crypto.randomUUID();
    context.store.database
      .query(
        `INSERT INTO team_review_packets
          (id, operation_id, team_id, generation, rule_id, rule_version, actor,
           work_ref, decision_ref, authority_ref, stop_ref, evidence, excerpt,
           truncated, health_receipt_id, dedupe_key, capability, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED', ?)`,
      )
      .run(
        id,
        operationId,
        teamId,
        snapshot.generation,
        bounded.ruleId,
        bounded.ruleVersion,
        bounded.actor,
        bounded.workRef,
        bounded.decisionRef,
        bounded.authorityRef,
        bounded.stopRef,
        bounded.evidence,
        bounded.excerpt,
        bounded.truncated ? 1 : 0,
        bounded.healthReceiptId,
        bounded.dedupeKey,
        capability,
        createdAt,
      );
    row = findPacket(context, id) as ReviewPacketRow;
  }
  const packet = packetFromRow(row);
  let delivery: RuntimeEffect;
  try {
    delivery = await runtime.deliverObserver(
      plan,
      packetPrompt(packet),
      `packet.${packet.id}.deliver`,
      (effect) => recordEffect(context, operationId, effect),
    );
  } catch (error) {
    delivery = {
      data: { error: error instanceof Error ? error.message : String(error) },
      key: `packet.${packet.id}.deliver`,
      kind: "agent.prompt",
      ok: false,
      resourceKey: plan.sensorResourceKey,
    };
    recordEffect(context, operationId, delivery);
  }
  const missing: MissingPostcondition[] = delivery.ok
    ? []
    : [{
        actual: delivery.data,
        code: "observer.delivery",
        expected: { delivered: true, packetId: packet.id },
        resource: plan.sensorResourceKey,
      }];
  const axes: TeamAxes = delivery.ok
    ? { health: snapshot.health, review: snapshot.review, stage: snapshot.stage }
    : { health: "DEGRADED", review: snapshot.review, stage: snapshot.stage };
  const effects = [
    ...snapshot.resources.filter((effect) => effect.key !== delivery.key),
    delivery,
  ];
  const deliveredAt = new Date().toISOString();
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes,
    effects,
    inspection: receiptInspection({
      actual: { delivery: delivery.data, packetId: packet.id },
      complete: delivery.ok,
      missing,
      revision: delivery.ok ? `packet:${packet.id}:delivered` : `packet:${packet.id}:failed`,
    }),
    operationId,
    plan,
    previous: snapshot,
    result: delivery.ok ? "PACKET_DELIVERED" : "DELIVERY_FAILED",
    transactionMutation: () => {
      context.store.database
        .query("UPDATE team_review_packets SET status = ?, delivered_at = ? WHERE id = ?")
        .run(delivery.ok ? "DELIVERED" : "DELIVERY_FAILED", deliveredAt, packet.id);
    },
  });
  const finalizedPacket = packetFromRow(findPacket(context, packet.id) as ReviewPacketRow);
  if (!delivery.ok) {
    throw new CliError(
      "OBSERVER_DELIVERY_FAILED",
      `${packet.id} could not be delivered to observer-${teamId}`,
      { packet: finalizedPacket, receipt: outcome.receipt, team: outcome.team },
    );
  }
  return {
    data: { deduped: false, packet: finalizedPacket, receipt: outcome.receipt, team: outcome.team },
    text: `${packet.id} delivered: ${rule.id}@${rule.version}`,
  };
}

function reviewAxisOperation(
  context: PluginContext,
  input: {
    axes: TeamAxes;
    kind: string;
    operationId: string;
    override?: TeamAuthorityOverride | null;
    requestedBy: string;
    result: string;
    snapshot: TeamSnapshot;
    transactionMutation?: () => void;
  },
): { receipt: TeamReceipt; team: TeamSnapshot } {
  const existing = findReceipt(context, input.operationId);
  if (existing) {
    validateReceiptIdentity(existing, input.kind, input.snapshot.teamId);
    if (existing.status === "FINALIZED") {
      return { receipt: receiptFromRow(existing), team: requireSnapshot(context, input.snapshot.teamId) };
    }
  }
  const plan = planForSnapshot(context, input.snapshot);
  if (!existing) {
    insertAttempt(context, {
      desired: { plan },
      generation: input.snapshot.generation,
      kind: input.kind,
      operationId: input.operationId,
      override: input.override,
      requestedBy: input.requestedBy,
      snapshot: input.snapshot,
      teamId: input.snapshot.teamId,
    });
  }
  return finalizeReceiptAndSnapshot(context, {
    axes: input.axes,
    effects: input.snapshot.resources,
    inspection: receiptInspection({
      actual: { axis: input.kind, requestedBy: input.requestedBy },
      revision: `${input.kind}:${input.operationId}`,
    }),
    operationId: input.operationId,
    plan,
    previous: input.snapshot,
    result: input.result,
    transactionMutation: input.transactionMutation,
  });
}

function raiseReview(context: PluginContext, invocation: CliInvocation): CliResult {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existing = findReceipt(context, operationId);
  if (existing?.status === "FINALIZED") {
    validateReceiptIdentity(existing, "team.review.raise", teamId);
    return {
      data: { receipt: receiptFromRow(existing), team: requireSnapshot(context, teamId) },
      text: `${teamId} REVIEW_REQUIRED`,
    };
  }
  const snapshot = requireSnapshot(context, teamId);
  const packetId = requiredStringOption(invocation, "packet");
  const packet = findPacket(context, packetId);
  if (
    !packet ||
    packet.team_id !== teamId ||
    packet.generation !== snapshot.generation ||
    packet.status !== "DELIVERED" ||
    packet.capability !== requiredStringOption(invocation, "capability")
  ) {
    throw new CliError(
      "OBSERVER_CAPABILITY_REJECTED",
      `${packetId} is not a live packet capability for ${teamId} generation ${snapshot.generation}`,
      { generation: snapshot.generation, packetId, teamId },
    );
  }
  const finding = requiredStringOption(invocation, "finding");
  const outcome = reviewAxisOperation(context, {
    axes: { health: snapshot.health, review: "REVIEW_REQUIRED", stage: snapshot.stage },
    kind: "team.review.raise",
    operationId,
    requestedBy: `observer-${teamId}`,
    result: "REVIEW_REQUIRED",
    snapshot,
    transactionMutation: () => {
      context.store.database
        .query(
          `UPDATE team_review_packets
           SET status = 'VERDICTED', verdict = 'REVIEW_REQUIRED', finding = ?
           WHERE id = ? AND status = 'DELIVERED'`,
        )
        .run(finding, packetId);
    },
  });
  return {
    data: {
      packet: packetFromRow(findPacket(context, packetId) as ReviewPacketRow),
      receipt: outcome.receipt,
      team: outcome.team,
    },
    text: `${teamId} REVIEW_HOLD from ${packetId}`,
  };
}

function assertTeamSupervisor(teamId: string, requestedBy: string): void {
  const expected = `supervisor-${teamId}`;
  if (requestedBy !== expected) {
    throw new CliError(
      "TEAM_AUTHORITY_REQUIRED",
      `${expected} must authorize this active-team operation; got ${requestedBy}`,
      { expected, requestedBy, teamId },
    );
  }
}

function authorityOverrideFromReceipt(row: ReceiptRow): TeamAuthorityOverride | null {
  const evidenceJson = row.override_evidence_json ?? null;
  const reason = row.override_reason ?? null;
  if (reason === null && evidenceJson === null) return null;
  if (reason === null || evidenceJson === null) {
    throw new CliError(
      "STORE_CORRUPT",
      `${row.operation_id} has incomplete team override authority`,
      { operationId: row.operation_id },
    );
  }
  return {
    evidence: json<TeamOverrideEvidence>(evidenceJson, {} as TeamOverrideEvidence),
    reason,
  };
}

function hasOverrideOptions(invocation: CliInvocation): boolean {
  return invocation.options["override-reason"] !== undefined ||
    invocation.options["override-evidence"] !== undefined ||
    invocation.options["owner-intervention"] !== undefined;
}

async function authorizeActiveTeamMutation(
  context: PluginContext,
  runtime: TeamRuntime,
  snapshot: TeamSnapshot,
  invocation: CliInvocation,
  existing: ReceiptRow | null,
): Promise<{ override: TeamAuthorityOverride | null; requestedBy: string }> {
  const teamId = snapshot.teamId;
  const requestedBy = requiredStringOption(invocation, "requested-by");
  const expected = `supervisor-${teamId}`;
  const ownerIntervention = invocation.options["owner-intervention"] === true;

  if (existing) {
    if (existing.requested_by !== requestedBy) {
      throw new CliError(
        "OPERATION_CONFLICT",
        `${existing.operation_id} was authorized by ${existing.requested_by}, not ${requestedBy}`,
        {
          attemptedRequestedBy: existing.requested_by,
          operationId: existing.operation_id,
          requestedBy,
        },
      );
    }
    const override = authorityOverrideFromReceipt(existing);
    if (!override) {
      if (hasOverrideOptions(invocation)) {
        throw new CliError(
          "OPERATION_CONFLICT",
          `${existing.operation_id} was attempted without emergency override authority`,
          { operationId: existing.operation_id },
        );
      }
      assertTeamSupervisor(teamId, requestedBy);
      return { override: null, requestedBy };
    }
    const reason = requiredStringOption(invocation, "override-reason");
    const declared = requiredStringOption(invocation, "override-evidence");
    const expectedOwnerIntervention = override.evidence.basis === "owner-intervention";
    if (
      override.reason !== reason ||
      override.evidence.declared !== declared ||
      ownerIntervention !== expectedOwnerIntervention
    ) {
      throw new CliError(
        "OPERATION_CONFLICT",
        `${existing.operation_id} override authority does not match its attempted receipt`,
        { operationId: existing.operation_id },
      );
    }
    return { override, requestedBy };
  }

  if (requestedBy === expected) {
    if (hasOverrideOptions(invocation)) {
      throw new CliError(
        "INVALID_ARGUMENT",
        `${expected} must use routine authority without emergency override flags`,
        { requestedBy, teamId },
      );
    }
    return { override: null, requestedBy };
  }

  if (requestedBy === "owner") {
    if (!ownerIntervention) {
      throw new CliError(
        "TEAM_OVERRIDE_DENIED",
        "owner emergency authority requires --owner-intervention",
        { requestedBy, teamId },
      );
    }
    const reason = requiredStringOption(invocation, "override-reason");
    const declared = requiredStringOption(invocation, "override-evidence");
    return {
      override: {
        evidence: {
          basis: "owner-intervention",
          declared,
          inspectedAt: null,
          missing: [],
          runtimeRevision: null,
        },
        reason,
      },
      requestedBy,
    };
  }

  if (requestedBy !== "supervisor") {
    assertTeamSupervisor(teamId, requestedBy);
  }
  if (ownerIntervention) {
    throw new CliError(
      "TEAM_OVERRIDE_DENIED",
      "--owner-intervention requires --requested-by owner",
      { requestedBy, teamId },
    );
  }
  const reason = requiredStringOption(invocation, "override-reason");
  const declared = requiredStringOption(invocation, "override-evidence");
  const plan = planForSnapshot(context, snapshot);
  let inspection: TeamInspection;
  try {
    inspection = await runtime.inspect(plan, snapshot.resources);
  } catch (error) {
    const failed = runtimeFailureInspection(error);
    throw new CliError(
      "TEAM_OVERRIDE_UNPROVEN",
      `cannot prove supervisor-${teamId} absent or unreachable`,
      {
        missing: failed.missing,
        requestedBy,
        supervisorReachable: null,
        teamId,
      },
    );
  }
  const supervisorResource = plan.roles.find((role) => role.role === "supervisor")?.resourceKey;
  const unreachableCodes = new Set(["role.missing", "role.pane", "role.process", "role.unreachable"]);
  const missing = inspection.missing.filter(
    (item) => item.resource === supervisorResource && unreachableCodes.has(item.code),
  );
  if (missing.length === 0) {
    throw new CliError(
      "TEAM_OVERRIDE_DENIED",
      `supervisor-${teamId} is reachable; Room emergency override is not permitted`,
      {
        requestedBy,
        supervisorReachable: true,
        teamId,
      },
    );
  }
  return {
    override: {
      evidence: {
        basis: "supervisor-unreachable",
        declared,
        inspectedAt: inspection.inspectedAt,
        missing,
        runtimeRevision: inspection.runtimeRevision,
      },
      reason,
    },
    requestedBy,
  };
}

async function clearReview(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existing = findReceipt(context, operationId);
  if (existing?.status === "FINALIZED") {
    validateReceiptIdentity(existing, "team.review.clear", teamId);
    return {
      data: { receipt: receiptFromRow(existing), team: requireSnapshot(context, teamId) },
      text: `${teamId} review CLEAR`,
    };
  }
  const snapshot = requireSnapshot(context, teamId);
  const rationale = requiredStringOption(invocation, "rationale");
  if (snapshot.review !== "REVIEW_REQUIRED") {
    throw new CliError("INVALID_STATE", `${teamId} has no review hold to clear`, {
      review: snapshot.review,
      teamId,
    });
  }
  const authority = await authorizeActiveTeamMutation(
    context,
    runtime,
    snapshot,
    invocation,
    existing,
  );
  const resolvedAt = new Date().toISOString();
  const outcome = reviewAxisOperation(context, {
    axes: { health: snapshot.health, review: "CLEAR", stage: snapshot.stage },
    kind: "team.review.clear",
    operationId,
    override: authority.override,
    requestedBy: authority.requestedBy,
    result: "REVIEW_CLEAR",
    snapshot,
    transactionMutation: () => {
      context.store.database
        .query(
          `UPDATE team_review_packets
           SET status = 'RESOLVED', resolved_at = ?, finding = COALESCE(finding, ?)
           WHERE team_id = ? AND generation = ? AND status = 'VERDICTED'`,
        )
        .run(resolvedAt, rationale, teamId, snapshot.generation);
    },
  });
  return {
    data: { rationale, receipt: outcome.receipt, team: outcome.team },
    text: `${teamId} review CLEAR: ${rationale}`,
  };
}

async function escalateReview(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const baseOperationId = requiredOperation(invocation);
  const reviewOperationId = `${baseOperationId}:review`;
  const healthOperationId = `${baseOperationId}:health`;
  const existingReview = findReceipt(context, reviewOperationId);
  const existingHealth = findReceipt(context, healthOperationId);
  if (existingReview?.status === "FINALIZED" && existingHealth?.status === "FINALIZED") {
    validateReceiptIdentity(existingReview, "team.review.escalate.review", teamId);
    validateReceiptIdentity(existingHealth, "team.review.escalate.health", teamId);
    return {
      data: {
        receipts: [receiptFromRow(existingReview), receiptFromRow(existingHealth)],
        team: requireSnapshot(context, teamId),
      },
      text: `${teamId} review escalated to DEGRADED`,
    };
  }
  const rationale = requiredStringOption(invocation, "rationale");
  const before = requireSnapshot(context, teamId);
  if (before.review !== "REVIEW_REQUIRED") {
    throw new CliError("INVALID_STATE", `${teamId} has no review hold to escalate`, {
      review: before.review,
      teamId,
    });
  }
  const authority = await authorizeActiveTeamMutation(
    context,
    runtime,
    before,
    invocation,
    existingReview ?? existingHealth,
  );
  const resolvedAt = new Date().toISOString();
  const reviewOutcome = reviewAxisOperation(context, {
    axes: { health: before.health, review: "CLEAR", stage: before.stage },
    kind: "team.review.escalate.review",
    operationId: reviewOperationId,
    override: authority.override,
    requestedBy: authority.requestedBy,
    result: "REVIEW_ESCALATED",
    snapshot: before,
    transactionMutation: () => {
      context.store.database
        .query(
          `UPDATE team_review_packets
           SET status = 'RESOLVED', resolved_at = ?, finding = COALESCE(finding, ?)
           WHERE team_id = ? AND generation = ? AND status = 'VERDICTED'`,
        )
        .run(resolvedAt, rationale, teamId, before.generation);
    },
  });
  const healthOutcome = reviewAxisOperation(context, {
    axes: { health: "DEGRADED", review: reviewOutcome.team.review, stage: reviewOutcome.team.stage },
    kind: "team.review.escalate.health",
    operationId: healthOperationId,
    override: authority.override,
    requestedBy: authority.requestedBy,
    result: "DEGRADED",
    snapshot: reviewOutcome.team,
  });
  return {
    data: {
      rationale,
      receipts: [reviewOutcome.receipt, healthOutcome.receipt],
      team: healthOutcome.team,
    },
    text: `${teamId} review escalated to DEGRADED: ${rationale}`,
  };
}

async function adviseTeam(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existingReceipt = findReceipt(context, operationId);
  if (existingReceipt) {
    validateReceiptIdentity(existingReceipt, "team.advise", teamId);
    const existingConsultation = findConsultation(context, operationId);
    if (existingReceipt.status === "FINALIZED" && existingConsultation) {
      const consultation = consultationFromRow(existingConsultation);
      const team = requireSnapshot(context, teamId);
      if (consultation.status === "FAILED") {
        throw new CliError(
          "ADVISOR_FAILED",
          consultation.error ?? "Advisor consultation failed",
          { consultation, receipt: receiptFromRow(existingReceipt), team },
        );
      }
      return {
        data: { consultation, receipt: receiptFromRow(existingReceipt), team },
        text: `${operationId} Advisor: ${consultation.recommendation}`,
      };
    }
  }
  const snapshot = requireSnapshot(context, teamId);
  if (snapshot.stage !== "ACTIVE" || snapshot.health !== "READY") {
    throw new CliError(
      "INVALID_STATE",
      `${teamId} is ${snapshot.stage}/${snapshot.health ?? "UNPROVEN"}; Advisor requires live baseline roles`,
      { health: snapshot.health, stage: snapshot.stage, teamId },
    );
  }
  const plan = planForSnapshot(context, snapshot);
  const requestedBy = requiredStringOption(invocation, "requested-by");
  const lead = plan.roles.find((role) => role.role === "lead")?.agentName;
  if (requestedBy !== `supervisor-${teamId}` && requestedBy !== lead) {
    throw new CliError(
      "TEAM_AUTHORITY_REQUIRED",
      `Advisor may be requested only by supervisor-${teamId} or ${lead ?? "the registered Lead"}`,
      { lead: lead ?? null, requestedBy, supervisor: `supervisor-${teamId}`, teamId },
    );
  }
  const timeoutMs = nonNegativeIntegerOption(invocation, "timeout-ms", 120_000, 300_000);
  if (timeoutMs <= 3_000) {
    throw new CliError("INVALID_ARGUMENT", "--timeout-ms must be greater than 3000", {
      timeoutMs,
    });
  }
  const request: AdvisorConsultationRequest = {
    contextRefs: stringOptions(invocation, "context"),
    decisionRef: requiredStringOption(invocation, "decision"),
    operationId,
    question: requiredStringOption(invocation, "question"),
    requestedBy,
    stopCondition: requiredStringOption(invocation, "stop-condition"),
    timeoutMs,
  };
  if (!existingReceipt) {
    insertAttempt(context, {
      desired: { plan },
      generation: snapshot.generation,
      kind: "team.advise",
      operationId,
      requestedBy,
      snapshot,
      teamId,
    });
  }
  let consultationRow = findConsultation(context, operationId);
  if (!consultationRow) {
    const createdAt = new Date().toISOString();
    context.store.database
      .query(
        `INSERT INTO team_advisor_consultations
          (operation_id, team_id, generation, requested_by, decision_ref, question,
           context_json, stop_condition, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ATTEMPTED', ?)`,
      )
      .run(
        operationId,
        teamId,
        snapshot.generation,
        requestedBy,
        request.decisionRef,
        request.question,
        JSON.stringify(request.contextRefs),
        request.stopCondition,
        createdAt,
      );
    consultationRow = findConsultation(context, operationId) as AdvisorConsultationRow;
  }
  const runtimeResult = await runtime.consultAdvisor(
    plan,
    request,
    latestEffects(context, operationId),
    (effect) => recordEffect(context, operationId, effect),
  );
  const succeeded = Boolean(runtimeResult.recommendation && runtimeResult.stopped && !runtimeResult.error);
  const completedAt = new Date().toISOString();
  const missing: MissingPostcondition[] = succeeded
    ? []
    : [{
        actual: {
          error: runtimeResult.error,
          recommendation: runtimeResult.recommendation,
          stopped: runtimeResult.stopped,
        },
        code: "advisor.consultation",
        expected: { recommendation: "non-empty", stopped: true },
        resource: `team:${teamId}:g${snapshot.generation}:advisor`,
      }];
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes: { health: snapshot.health, review: snapshot.review, stage: snapshot.stage },
    effects: snapshot.resources,
    inspection: receiptInspection({
      actual: {
        error: runtimeResult.error,
        paneId: runtimeResult.paneId,
        recommendation: runtimeResult.recommendation,
        stopped: runtimeResult.stopped,
        tabId: runtimeResult.tabId,
      },
      complete: succeeded,
      missing,
      revision: `advisor:${operationId}:${succeeded ? "completed" : "failed"}`,
    }),
    operationId,
    plan,
    previous: snapshot,
    result: succeeded ? "ADVISED" : "ADVISOR_FAILED",
    transactionMutation: () => {
      context.store.database
        .query(
          `UPDATE team_advisor_consultations
           SET status = ?, recommendation = ?, error = ?, completed_at = ?
           WHERE operation_id = ?`,
        )
        .run(
          succeeded ? "COMPLETED" : "FAILED",
          runtimeResult.recommendation,
          runtimeResult.error,
          completedAt,
          operationId,
        );
    },
  });
  const consultation = consultationFromRow(
    findConsultation(context, operationId) as AdvisorConsultationRow,
  );
  if (!succeeded) {
    throw new CliError(
      "ADVISOR_FAILED",
      runtimeResult.error ?? "Advisor consultation failed without a recommendation",
      { consultation, receipt: outcome.receipt, team: outcome.team },
    );
  }
  return {
    data: { consultation, receipt: outcome.receipt, team: outcome.team },
    text: `${operationId} Advisor: ${runtimeResult.recommendation}`,
  };
}

async function bindTeamProject(
  context: PluginContext,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existing = context.store.database
    .query<BindingReceiptRow, [string]>(
      "SELECT * FROM team_binding_receipts WHERE operation_id = ?",
    )
    .get(operationId);
  if (existing) {
    if (existing.team_id !== teamId) {
      throw new CliError(
        "OPERATION_CONFLICT",
        `${operationId} already binds project ${existing.project_root} to ${existing.team_id}`,
      );
    }
    if (existing.status === "FINALIZED" && existing.binding_json) {
      const binding = json<Record<string, unknown>>(existing.binding_json, {});
      return {
        data: { binding, receipt: existing },
        text: `${teamId} bound: ${existing.project_root}`,
      };
    }
  }
  const snapshot = requireSnapshot(context, teamId);
  const requestedBy = requiredStringOption(invocation, "requested-by");
  assertTeamSupervisor(teamId, requestedBy);
  const projectRoot = resolve(requiredStringOption(invocation, "repo"));
  let directory = false;
  try {
    directory = (await stat(projectRoot)).isDirectory();
  } catch {}
  if (!directory) {
    throw new CliError("NOT_FOUND", `project path not found: ${projectRoot}`, { projectRoot });
  }
  if (!existing) {
    context.store.database
      .query(
        `INSERT INTO team_binding_receipts
          (operation_id, team_id, generation, project_root, requested_by, executed_by,
           status, attempted_at)
         VALUES (?, ?, ?, ?, ?, ?, 'ATTEMPTED', ?)`,
      )
      .run(
        operationId,
        teamId,
        snapshot.generation,
        projectRoot,
        requestedBy,
        context.sessions.current().id,
        new Date().toISOString(),
      );
  }
  try {
    const binding = bindProject({
      generation: snapshot.generation,
      projectRoot,
      roomRoot: process.cwd(),
      roomStore: context.store,
      teamId,
    });
    const completedAt = new Date().toISOString();
    context.store.database
      .query(
        `UPDATE team_binding_receipts
         SET status = 'FINALIZED', binding_json = ?, completed_at = ?
         WHERE operation_id = ? AND status = 'ATTEMPTED'`,
      )
      .run(JSON.stringify(binding), completedAt, operationId);
    const receipt = context.store.database
      .query<BindingReceiptRow, [string]>(
        "SELECT * FROM team_binding_receipts WHERE operation_id = ?",
      )
      .get(operationId);
    context.log.append({
      entityId: teamId,
      entityType: "team",
      payload: { bindingId: binding.bindingId, generation: snapshot.generation, projectRoot },
      sessionId: context.sessions.current().id,
      type: "team.bind",
    });
    return {
      data: { binding, receipt },
      text: `${teamId} bound: ${projectRoot}`,
    };
  } catch (error) {
    context.store.database
      .query(
        `UPDATE team_binding_receipts
         SET status = 'FAILED', error = ?, completed_at = ?
         WHERE operation_id = ? AND status = 'ATTEMPTED'`,
      )
      .run(
        error instanceof Error ? error.message : String(error),
        new Date().toISOString(),
        operationId,
      );
    throw new CliError(
      "TEAM_BINDING_FAILED",
      `cannot bind ${projectRoot} to ${teamId}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function reconcileTeam(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existing = findReceipt(context, operationId);
  if (existing) {
    validateReceiptIdentity(existing, "team.reconcile", teamId);
    if (existing.status === "FINALIZED") {
      const receipt = receiptFromRow(existing);
      const team = requireSnapshot(context, teamId);
      if (receipt.result === "RECONCILE_INCOMPLETE") {
        throw new CliError(
          "TEAM_RECONCILE_INCOMPLETE",
          `${teamId} still misses ${receipt.missing.length} postconditions`,
          { receipt, team },
        );
      }
      return {
        data: { receipt, team },
        text: `${teamId} reconciled: ${team.verdict}`,
      };
    }
  }
  const snapshot = requireSnapshot(context, teamId);
  if (snapshot.stage !== "ACTIVE" && snapshot.stage !== "STARTING") {
    throw new CliError("INVALID_STATE", `${teamId} is ${snapshot.stage}`, {
      stage: snapshot.stage,
      teamId,
    });
  }
  const rawResources = [...new Set(stringOptions(invocation, "resource"))];
  const allowedResources = new Set<TeamRepairResource>([
    "workspace",
    "supervisor",
    "lead",
    "observer",
    "sensor",
  ]);
  if (rawResources.length === 0) {
    throw new CliError("MISSING_ARGUMENT", "pass at least one --resource to reconcile");
  }
  const invalid = rawResources.filter(
    (resource): resource is string => !allowedResources.has(resource as TeamRepairResource),
  );
  if (invalid.length > 0) {
    throw new CliError(
      "INVALID_ARGUMENT",
      `unknown reconcile resource: ${invalid.join(", ")}`,
      { allowed: [...allowedResources], invalid },
    );
  }
  const resources = rawResources as TeamRepairResource[];
  const authority = await authorizeActiveTeamMutation(
    context,
    runtime,
    snapshot,
    invocation,
    existing,
  );
  const plan = planForSnapshot(context, snapshot);
  if (!existing) {
    insertAttempt(context, {
      desired: { plan },
      generation: snapshot.generation,
      kind: "team.reconcile",
      operationId,
      override: authority.override,
      requestedBy: authority.requestedBy,
      snapshot,
      teamId,
    });
  }
  let effects = snapshot.resources;
  let inspection: TeamInspection;
  try {
    effects = await runtime.reconcile(
      plan,
      resources,
      effects,
      (effect) => recordEffect(context, operationId, effect),
    );
    inspection = await runtime.inspect(plan, effects);
  } catch (error) {
    effects = [
      ...snapshot.resources.filter(
        (effect) => !latestEffects(context, operationId).some((current) => current.key === effect.key),
      ),
      ...latestEffects(context, operationId),
    ];
    inspection = runtimeFailureInspection(error);
  }
  const axes: TeamAxes = inspection.complete
    ? { health: "READY", review: snapshot.review, stage: "ACTIVE" }
    : snapshot.stage === "STARTING"
      ? { health: null, review: snapshot.review, stage: "STARTING" }
      : { health: "DEGRADED", review: snapshot.review, stage: "ACTIVE" };
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes,
    effects,
    inspection,
    operationId,
    plan,
    previous: snapshot,
    result: inspection.complete ? "RECONCILED" : "RECONCILE_INCOMPLETE",
  });
  context.log.append({
    entityId: teamId,
    entityType: "team",
    payload: {
      missing: inspection.missing,
      operationId,
      resources,
      result: outcome.receipt.result,
    },
    sessionId: context.sessions.current().id,
    type: "team.reconcile",
  });
  if (!inspection.complete) {
    throw new CliError(
      "TEAM_RECONCILE_INCOMPLETE",
      `${teamId} still misses ${inspection.missing.length} postconditions after selected repair`,
      { receipt: outcome.receipt, team: outcome.team },
    );
  }
  return {
    data: { receipt: outcome.receipt, team: outcome.team },
    text: `${teamId} reconciled: ${outcome.team.verdict}`,
  };
}

interface ProjectBindingRow {
  binding_id: string;
  project_root: string;
  project_store_path: string;
}

function mergeRuntimeEffects(...groups: readonly RuntimeEffect[][]): RuntimeEffect[] {
  const effects = new Map<string, RuntimeEffect>();
  for (const group of groups) {
    for (const effect of group) effects.set(effect.key, effect);
  }
  return [...effects.values()];
}

function tableExists(store: Store, name: string): boolean {
  return store.database
    .query<{ name: string }, [string]>(
      "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .get(name) !== null;
}

function projectDrainBlockers(
  context: PluginContext,
  snapshot: TeamSnapshot,
): MissingPostcondition[] {
  const blockers: MissingPostcondition[] = [];
  const bindings = context.store.database
    .query<ProjectBindingRow, [string, number]>(
      `SELECT binding_id, project_root, project_store_path
       FROM team_project_bindings
       WHERE team_id = ? AND generation = ? AND status = 'ACTIVE'
       ORDER BY project_root`,
    )
    .all(snapshot.teamId, snapshot.generation);
  for (const binding of bindings) {
    const shared = resolve(binding.project_store_path) === resolve(context.store.path);
    let projectStore: Store | null = null;
    try {
      projectStore = shared ? context.store : new Store(binding.project_store_path, { readonly: true });
      if (tableExists(projectStore, "work")) {
        const workRows = projectStore.database
          .query<{
            held_by: string | null;
            id: string;
            state: string;
          }, []>(
            `SELECT id, state, held_by
             FROM work
             WHERE cancelled_at IS NULL
               AND state != 'done'
               AND (state = 'active' OR held_by IS NOT NULL)
             ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)`,
          )
          .all();
        for (const work of workRows) {
          if (work.held_by) {
            blockers.push({
              actual: {
                bindingId: binding.binding_id,
                heldBy: work.held_by,
                projectRoot: binding.project_root,
                state: work.state,
              },
              code: "shutdown.lease",
              expected: "work lease released",
              resource: `work:${work.id}`,
            });
          } else {
            blockers.push({
              actual: {
                bindingId: binding.binding_id,
                projectRoot: binding.project_root,
                state: work.state,
              },
              code: "shutdown.work",
              expected: "active work settled",
              resource: `work:${work.id}`,
            });
          }
        }
      }
      if (tableExists(projectStore, "dispatches") && tableExists(projectStore, "handbacks")) {
        const dispatchRows = projectStore.database
          .query<{
            held_by: string | null;
            id: string;
            pane: string | null;
            target_session: string | null;
            work_id: string;
          }, []>(
            `SELECT d.id, d.work_id, d.held_by, d.pane, d.target_session
             FROM dispatches d
             LEFT JOIN handbacks h ON h.dispatch_id = d.id
             WHERE d.cancelled_at IS NULL AND h.id IS NULL
             ORDER BY CAST(SUBSTR(d.id, 2) AS INTEGER)`,
          )
          .all();
        for (const dispatch of dispatchRows) {
          blockers.push({
            actual: {
              bindingId: binding.binding_id,
              heldBy: dispatch.held_by,
              pane: dispatch.pane,
              projectRoot: binding.project_root,
              targetSession: dispatch.target_session,
              workId: dispatch.work_id,
            },
            code: "shutdown.handback",
            expected: "handback filed or dispatch cancelled",
            resource: `dispatch:${dispatch.id}`,
          });
        }
      }
    } catch (error) {
      blockers.push({
        actual: {
          bindingId: binding.binding_id,
          error: error instanceof Error ? error.message : String(error),
          projectRoot: binding.project_root,
          projectStorePath: binding.project_store_path,
        },
        code: "shutdown.project-store",
        expected: "readable bound project store",
        resource: `binding:${binding.binding_id}`,
      });
    } finally {
      if (projectStore && !shared) projectStore.close();
    }
  }
  const activeAdvisors = context.store.database
    .query<{ operation_id: string; requested_by: string }, [string, number]>(
      `SELECT operation_id, requested_by
       FROM team_advisor_consultations
       WHERE team_id = ? AND generation = ? AND status = 'RUNNING'
       ORDER BY created_at`,
    )
    .all(snapshot.teamId, snapshot.generation);
  for (const advisor of activeAdvisors) {
    blockers.push({
      actual: { operationId: advisor.operation_id, requestedBy: advisor.requested_by },
      code: "shutdown.advisor",
      expected: "Advisor consultation returned and stopped",
      resource: `advisor:${advisor.operation_id}`,
    });
  }
  return blockers;
}

function drainDeliveryBlockers(
  effects: readonly RuntimeEffect[],
  operationId: string,
): MissingPostcondition[] {
  const prefix = `shutdown.${operationId}.drain.`;
  return effects
    .filter((effect) => effect.key.startsWith(prefix) && !effect.ok)
    .map((effect) => ({
      actual: effect.data,
      code: "shutdown.drain-delivery",
      expected: "bounded drain notice delivered",
      resource: effect.resourceKey,
    }));
}

async function awaitProjectDrain(
  context: PluginContext,
  snapshot: TeamSnapshot,
  fixed: readonly MissingPostcondition[],
  waitMs: number,
): Promise<MissingPostcondition[]> {
  const deadline = Date.now() + waitMs;
  while (true) {
    const blockers = [...fixed, ...projectDrainBlockers(context, snapshot)];
    if (blockers.length === 0 || Date.now() >= deadline) return blockers;
    await Bun.sleep(Math.min(100, Math.max(1, deadline - Date.now())));
  }
}

function combineShutdownInspection(
  runtime: TeamInspection,
  blockers: readonly MissingPostcondition[],
  actual: Record<string, unknown>,
): TeamInspection {
  return {
    actual: { ...runtime.actual, ...actual },
    complete: runtime.complete && blockers.length === 0,
    inspectedAt: runtime.inspectedAt,
    missing: [...blockers, ...runtime.missing],
    runtimeRevision: runtime.runtimeRevision,
  };
}

async function stopTeam(
  context: PluginContext,
  runtime: TeamRuntime,
  invocation: CliInvocation,
): Promise<CliResult> {
  const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
  const operationId = requiredOperation(invocation);
  const existing = findReceipt(context, operationId);
  if (existing) {
    validateReceiptIdentity(existing, "team.stop", teamId);
    if (existing.status === "FINALIZED") {
      const receipt = receiptFromRow(existing);
      const team = requireSnapshot(context, teamId);
      if (["STOP_TIMEOUT", "STOP_INCOMPLETE", "FORCED_STOP_INCOMPLETE"].includes(receipt.result ?? "")) {
        throw new CliError(
          "TEAM_STOPPING",
          `${teamId} stop did not prove generation absence`,
          { receipt, team },
        );
      }
      return {
        data: { receipt, team },
        text: `${teamId} ${receipt.result}`,
      };
    }
  }

  let snapshot = requireSnapshot(context, teamId);
  if (snapshot.stage === "STOPPED") {
    throw new CliError("INVALID_STATE", `${teamId} is already STOPPED`, {
      generation: snapshot.generation,
      teamId,
    });
  }
  if (!["STARTING", "ACTIVE", "STOPPING"].includes(snapshot.stage)) {
    throw new CliError("INVALID_STATE", `${teamId} is ${snapshot.stage}`, {
      stage: snapshot.stage,
      teamId,
    });
  }
  const priorStage = findReceipt(context, `${operationId}:stage`);
  if (priorStage) validateReceiptIdentity(priorStage, "team.stop.stage", teamId);
  const resumingStagedOperation =
    !existing &&
    priorStage?.status === "FINALIZED" &&
    priorStage.generation === snapshot.generation &&
    snapshot.stage === "STOPPING";
  if (!existing && !resumingStagedOperation) {
    const expectedRevision = nonNegativeIntegerOption(
      invocation,
      "expected-revision",
      snapshot.revision,
      Number.MAX_SAFE_INTEGER,
    );
    if (expectedRevision !== snapshot.revision) {
      throw new CliError(
        "STALE_REVISION",
        `expected revision ${expectedRevision} does not match current team revision ${snapshot.revision}`,
        { actualRevision: snapshot.revision, expectedRevision },
      );
    }
  }
  const forced = invocation.options.force === true;
  const forceReason = forced ? requiredStringOption(invocation, "reason") : null;
  const forceEvidence = forced ? requiredStringOption(invocation, "evidence") : null;
  if (!forced && (stringOption(invocation, "reason") || stringOption(invocation, "evidence"))) {
    throw new CliError("INVALID_ARGUMENT", "--reason and --evidence require --force");
  }
  if (existing && existing.generation !== snapshot.generation) {
    throw new CliError(
      "STALE_GENERATION",
      `${operationId} belongs to generation ${existing.generation}, current generation is ${snapshot.generation}`,
      { currentGeneration: snapshot.generation, operationGeneration: existing.generation },
    );
  }
  const authority = await authorizeActiveTeamMutation(
    context,
    runtime,
    snapshot,
    invocation,
    existing ?? (resumingStagedOperation ? priorStage : null),
  );
  const requestedBy = authority.requestedBy;

  if (snapshot.stage !== "STOPPING") {
    const stageOperationId = `${operationId}:stage`;
    const stageReceipt = priorStage;
    if (!stageReceipt || stageReceipt.status !== "FINALIZED") {
      const stagePlan = stageReceipt
        ? json<DesiredOperation>(stageReceipt.desired_json, {} as DesiredOperation).plan
        : planForSnapshot(context, snapshot);
      if (!stageReceipt) {
        insertAttempt(context, {
          desired: { plan: stagePlan },
          generation: snapshot.generation,
          kind: "team.stop.stage",
          operationId: stageOperationId,
          override: authority.override,
          requestedBy,
          snapshot,
          teamId,
        });
      }
      snapshot = finalizeReceiptAndSnapshot(context, {
        axes: { health: null, review: snapshot.review, stage: "STOPPING" },
        effects: snapshot.resources,
        inspection: receiptInspection({
          actual: { gates: "CLOSED", stage: "STOPPING" },
          revision: `ledger:${teamId}:g${snapshot.generation}:stopping`,
        }),
        operationId: stageOperationId,
        plan: stagePlan,
        previous: snapshot,
        result: "STOPPING",
      }).team;
    } else {
      snapshot = requireSnapshot(context, teamId);
    }
  }

  const desired: DesiredOperation = existing
    ? json<DesiredOperation>(existing.desired_json, {} as DesiredOperation)
    : {
      ...(forced && forceReason && forceEvidence
        ? { force: { evidence: forceEvidence, reason: forceReason } }
        : {}),
      plan: planForSnapshot(context, snapshot),
    };
  if (existing && Boolean(desired.force) !== forced) {
    throw new CliError(
      "OPERATION_CONFLICT",
      `${operationId} force mode does not match its attempted receipt`,
      { attemptedForced: Boolean(desired.force), requestedForced: forced },
    );
  }
  if (!existing) {
    insertAttempt(context, {
      desired,
      generation: snapshot.generation,
      kind: "team.stop",
      operationId,
      override: authority.override,
      requestedBy,
      snapshot,
      teamId,
    });
  }

  let effects = mergeRuntimeEffects(snapshot.resources, latestEffects(context, operationId));
  let blockers: MissingPostcondition[] = [];
  if (forced) {
    blockers = projectDrainBlockers(context, snapshot);
  } else {
    let drainFailure: MissingPostcondition[] = [];
    try {
      effects = await runtime.requestDrain(
        desired.plan,
        operationId,
        effects,
        (effect) => recordEffect(context, operationId, effect),
      );
      drainFailure = drainDeliveryBlockers(effects, operationId);
    } catch (error) {
      drainFailure = runtimeFailureInspection(error).missing;
      effects = mergeRuntimeEffects(snapshot.resources, latestEffects(context, operationId));
    }
    blockers = await awaitProjectDrain(
      context,
      snapshot,
      drainFailure,
      nonNegativeIntegerOption(invocation, "wait-ms", 30_000, 600_000),
    );
    if (blockers.length > 0) {
      let presence: TeamInspection;
      try {
        presence = await runtime.inspectAbsence(desired.plan, effects);
      } catch (error) {
        presence = runtimeFailureInspection(error);
      }
      const inspection = combineShutdownInspection(presence, blockers, {
        drain: "timed out before runtime removal",
      });
      const outcome = finalizeReceiptAndSnapshot(context, {
        axes: { health: null, review: snapshot.review, stage: "STOPPING" },
        effects,
        inspection,
        operationId,
        plan: desired.plan,
        previous: snapshot,
        result: "STOP_TIMEOUT",
      });
      throw new CliError(
        "TEAM_STOPPING",
        `${teamId} remains STOPPING with ${inspection.missing.length} unsettled postconditions`,
        { receipt: outcome.receipt, team: outcome.team },
      );
    }
  }

  let inspection: TeamInspection;
  try {
    const shutdown = await runtime.shutdown(
      desired.plan,
      effects,
      (effect) => recordEffect(context, operationId, effect),
    );
    effects = shutdown.effects;
    inspection = combineShutdownInspection(
      shutdown.inspection,
      [],
      forced
        ? {
          evidence: forceEvidence,
          forceReason,
          possibleLoss: blockers,
        }
        : { graceful: true },
    );
  } catch (error) {
    effects = mergeRuntimeEffects(snapshot.resources, latestEffects(context, operationId));
    inspection = runtimeFailureInspection(error);
  }
  const stopped = inspection.complete;
  const result = stopped
    ? forced ? "FORCED_STOPPED" : "STOPPED"
    : forced ? "FORCED_STOP_INCOMPLETE" : "STOP_INCOMPLETE";
  const outcome = finalizeReceiptAndSnapshot(context, {
    axes: {
      health: null,
      review: snapshot.review,
      stage: stopped ? "STOPPED" : "STOPPING",
    },
    effects,
    forced,
    inspection,
    operationId,
    plan: desired.plan,
    previous: snapshot,
    result,
    transactionMutation: stopped
      ? () => {
        context.store.database
          .query(
            `UPDATE team_project_bindings
             SET status = 'STOPPED', updated_at = ?
             WHERE team_id = ? AND generation = ? AND status = 'ACTIVE'`,
          )
          .run(new Date().toISOString(), teamId, snapshot.generation);
      }
      : undefined,
  });
  context.log.append({
    entityId: teamId,
    entityType: "team",
    payload: {
      forced,
      missing: inspection.missing,
      operationId,
      possibleLoss: forced ? blockers : [],
      result,
    },
    sessionId: context.sessions.current().id,
    type: "team.stop",
  });
  if (!stopped) {
    throw new CliError(
      "TEAM_STOPPING",
      `${teamId} remains STOPPING; generation absence was not proved`,
      { receipt: outcome.receipt, team: outcome.team },
    );
  }
  return {
    data: { receipt: outcome.receipt, team: outcome.team },
    text: `${teamId} ${result}`,
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

const activeTeamAuthorityFlags = {
  ...commonMutationFlags,
  "--override-evidence": {
    description: "Cite evidence for a Room or owner emergency override.",
    value: true,
  },
  "--override-reason": {
    description: "Record why emergency override authority is necessary.",
    value: true,
  },
  "--owner-intervention": {
    description: "Record explicit owner intervention as emergency authority.",
  },
};

export const teamPlugin: BuiltInPlugin = {
  name: "team",
  apply(context) {
    migrate(context);
    const runtime: TeamRuntime = new HerdrTeamRuntime();
    context.effect(() => context.provide("teamRuntime", runtime));
    const teamControl = new TeamControl(context.store, context.sessions.current().id, runtime);
    context.effect(() => context.provide("teamControl", teamControl));
    const registerGate = (event: string, boundary: TeamControlBoundary): void => {
      context.effect(() =>
        context.events.on<Record<string, unknown>, { blocked: boolean; origin?: string; reason?: string }>(
          event,
          async (_input, next) => {
            const result = await teamControl.check(boundary);
            if (!result.bound || result.allowed) return next();
            return {
              blocked: true,
              origin: "team-control",
              reason: result.reason,
            };
          },
        ),
      );
    };
    registerGate("work.add", "work.add");
    registerGate("work.start", "work.start");
    registerGate("work.done", "work.done");
    registerGate("dispatch.open", "dispatch.open");
    registerGate("bundle.close", "bundle.close");
    registerGate("handback.final", "handback.final");
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
        "team bind",
        (invocation) => bindTeamProject(context, invocation),
        {
          description: "Bind another project to this Room-ledger team generation.",
          flags: {
            ...commonMutationFlags,
            "--repo": { description: "Select the project root to bind.", value: true },
          },
          json: true,
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
    context.effect(() =>
      context.cli.register(
        "team review",
        (invocation): CliResult => {
          const teamId = normalizeTeamId(requiredPosition(invocation, 0, "team id"));
          const team = requireSnapshot(context, teamId);
          const packets = context.store.database
            .query<ReviewPacketRow, [string, number]>(
              `SELECT * FROM team_review_packets
               WHERE team_id = ? AND generation = ?
               ORDER BY created_at`,
            )
            .all(teamId, team.generation)
            .map(packetFromRow);
          return {
            data: { packets, team },
            text: `${teamId} ${team.review}: ${packets.length} packet(s) in generation ${team.generation}`,
          };
        },
        {
          description: "Inspect packet-bound Observer review state.",
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
        "team review trigger",
        (invocation) => triggerReviewPacket(context, runtime, invocation),
        {
          description: "Deliver one threshold-validated bounded packet to Observer.",
          flags: {
            ...commonMutationFlags,
            "--actor": { description: "Name the role whose evidence fired the rule.", value: true },
            "--authority": { description: "Cite the relevant authority record.", value: true },
            "--decision": { description: "Cite the relevant decision record.", value: true },
            "--evidence": { description: "Record the exact matched evidence.", value: true },
            "--excerpt": { description: "Attach a capped surrounding excerpt.", value: true },
            "--occurrences": { description: "Record the count observed in the rule window.", value: true },
            "--rule": { description: "Select a versioned semantic core rule.", value: true },
            "--stop": { description: "Cite the bounded stop condition.", value: true },
            "--work": { description: "Cite the relevant work record.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team review spot-check",
        (invocation) => triggerReviewPacket(context, runtime, invocation, spotCheckRule),
        {
          description: "Send one generation- and window-bound Supervisor spot-check.",
          flags: {
            ...commonMutationFlags,
            "--authority": { description: "Cite the Supervisor authority record.", value: true },
            "--decision": { description: "Cite the relevant decision record.", value: true },
            "--excerpt": { description: "Attach a capped evidence-window excerpt.", value: true },
            "--question": { description: "Ask one bounded review question.", value: true },
            "--stop": { description: "Set the one-shot stop condition.", value: true },
            "--window": { description: "Name the evidence window.", value: true },
            "--work": { description: "Cite the relevant work record.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team review raise",
        (invocation) => raiseReview(context, invocation),
        {
          description: "Submit one capability-bound Observer verdict.",
          flags: {
            ...commonMutationFlags,
            "--capability": { description: "Present the packet's one-use capability.", value: true },
            "--finding": { description: "Record the bounded Observer finding.", value: true },
            "--packet": { description: "Name the delivered evidence packet.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team review clear",
        (invocation) => clearReview(context, runtime, invocation),
        {
          description: "Clear REVIEW_REQUIRED with team-Supervisor rationale.",
          flags: {
            ...activeTeamAuthorityFlags,
            "--rationale": { description: "Record why the finding is resolved.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team review escalate",
        (invocation) => escalateReview(context, runtime, invocation),
        {
          description: "Resolve review and record a separate DEGRADED health receipt.",
          flags: {
            ...activeTeamAuthorityFlags,
            "--rationale": { description: "Record why the review becomes a health failure.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team advise",
        (invocation) => adviseTeam(context, runtime, invocation),
        {
          description: "Run one bounded Advisor consultation.",
          flags: {
            ...commonMutationFlags,
            "--context": { description: "Cite a bounded context record.", multiple: true, value: true },
            "--decision": { description: "Bind the consultation to a decision record.", value: true },
            "--question": { description: "Ask one decision-focused question.", value: true },
            "--stop-condition": { description: "Define the Advisor's bounded stop.", value: true },
            "--timeout-ms": { description: "Bound the foreground consultation wait.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team reconcile",
        (invocation) => reconcileTeam(context, runtime, invocation),
        {
          description: "Repair explicitly selected team resources and re-prove readiness.",
          flags: {
            ...activeTeamAuthorityFlags,
            "--resource": {
              description: "Repair one named team resource.",
              multiple: true,
              value: true,
            },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
    context.effect(() =>
      registerSessionCommand(
        context,
        "team stop",
        (invocation) => stopTeam(context, runtime, invocation),
        {
          description: "Drain and stop one team generation.",
          flags: {
            ...activeTeamAuthorityFlags,
            "--evidence": { description: "Cite evidence authorizing possible loss.", value: true },
            "--force": { description: "Bypass unsettled work with a forced-loss receipt." },
            "--reason": { description: "Record why forced shutdown is necessary.", value: true },
            "--wait-ms": { description: "Bound the foreground drain wait.", value: true },
          },
          json: true,
          positionals: [{ name: "team-id", required: true }],
          rootDescription,
        },
      ),
    );
  },
};
