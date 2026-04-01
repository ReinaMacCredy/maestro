/**
 * Feature lifecycle usecases
 * Implements feature listing, updating, and worker report persistence
 */
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";
import type {
  Feature,
  FeatureStatus,
  UpdateFeatureInput,
  WorkerReport,
} from "../domain/mission-types.js";
import type { WorkerRuntime } from "../domain/runtime-types.js";
import { MaestroError } from "../domain/errors.js";
import { assertFeatureTransition } from "../domain/mission-state.js";
import { writeJson, readJson, ensureDir } from "../lib/fs.js";
import { join } from "node:path";
import {
  DEFAULT_RUNTIME_LEASE_MS,
  MAESTRO_DIR,
  UNKNOWN_AGENT,
} from "../domain/defaults.js";

/** Result of listing features */
export interface ListFeaturesResult {
  features: readonly Feature[];
  total: number;
  filtered: number;
}

/** Result of updating a feature */
export interface UpdateFeatureResult {
  feature: Feature;
  reportPersisted?: string; // path to persisted report
  missionAutoStarted?: boolean;
}

/**
 * List features for a mission with optional filters
 */
export async function listFeatures(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  missionId: string,
  filter?: { milestoneId?: string; status?: string },
): Promise<ListFeaturesResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  const features = await featureStore.list(missionId, {
    milestoneId: filter?.milestoneId,
    status: filter?.status,
  });

  const totalFeatures = await featureStore.list(missionId);

  return {
    features,
    total: totalFeatures.length,
    filtered: features.length,
  };
}

/**
 * Update a feature's status and/or report
 * Enforces legal state transitions and persists worker reports
 */
export async function updateFeature(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  runtimeStore: RuntimeStorePort,
  baseDir: string,
  missionId: string,
  featureId: string,
  input: UpdateFeatureInput,
): Promise<UpdateFeatureResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get existing feature
  const existing = await featureStore.get(missionId, featureId);
  if (!existing) {
    throw new MaestroError(`Feature ${featureId} not found in mission ${missionId}`, [
      `List features: maestro feature list --mission ${missionId}`,
      `Check that feature ID '${featureId}' is correct`,
    ]);
  }

  // Validate status transition if provided
  if (input.status !== undefined && input.status !== existing.status) {
    assertFeatureTransition(existing.status, input.status);
  }

  let missionAutoStarted = false;
  if (mission.status === "approved" && input.status !== undefined && input.status !== existing.status) {
    const autoStartedMission = await missionStore.update(missionId, { status: "executing" });
    if (!autoStartedMission) {
      throw new MaestroError(`Failed to auto-start mission ${missionId}`);
    }
    missionAutoStarted = true;
  }

  // Handle report persistence
  let reportPersisted: string | undefined;
  let finalReport: WorkerReport | undefined = input.report;

  // If no new report is provided but status is changing to pending (retry),
  // preserve the existing report
  if (input.report === undefined && input.status === "pending" && existing.report) {
    finalReport = existing.report;
  }

  // If a new report is provided, persist it to workers/{featureId}/report.json
  if (input.report !== undefined) {
    reportPersisted = await persistWorkerReport(baseDir, missionId, featureId, input.report);
  }

  // Persist retry reason if provided on retry (status -> pending)
  if (input.retryReason && input.status === "pending" && existing.status !== "pending") {
    const retryEntry = {
      reason: input.retryReason,
      timestamp: new Date().toISOString(),
      previousStatus: existing.status,
    };
    const workersDir = join(baseDir, MAESTRO_DIR, "missions", missionId, "workers", featureId);
    await ensureDir(workersDir);
    const retryLogPath = join(workersDir, "retry-log.json");
    const existingLog = await readJson<readonly unknown[]>(retryLogPath);
    const log = Array.isArray(existingLog) ? [...existingLog, retryEntry] : [retryEntry];
    await writeJson(retryLogPath, log);
  }

  // Update the feature
  const updateInput: UpdateFeatureInput = {
    status: input.status,
    report: finalReport,
  };

  const updated = await featureStore.update(missionId, featureId, updateInput);
  if (!updated) {
    throw new MaestroError(`Failed to update feature ${featureId}`);
  }

  await syncWorkerRuntime(
    runtimeStore,
    missionId,
    featureId,
    existing,
    updated,
    reportPersisted,
    input.retryReason,
  );

  return { feature: updated, reportPersisted, missionAutoStarted };
}

async function syncWorkerRuntime(
  runtimeStore: RuntimeStorePort,
  missionId: string,
  featureId: string,
  existing: Feature,
  updated: Feature,
  reportPersisted?: string,
  retryReason?: string,
): Promise<void> {
  const current = await runtimeStore.get(missionId, featureId);
  if (!current) return;

  const now = Date.now();
  const nowIso = new Date(now).toISOString();
  const nextRuntime = deriveNextRuntime(current, existing, updated, reportPersisted, retryReason, now, nowIso);
  await runtimeStore.save(missionId, featureId, nextRuntime);
}

function deriveNextRuntime(
  runtime: WorkerRuntime,
  existing: Feature,
  updated: Feature,
  reportPersisted: string | undefined,
  retryReason: string | undefined,
  now: number,
  nowIso: string,
): WorkerRuntime {
  const recoveryMetadata = {
    ...runtime.recoveryMetadata,
    history: [...runtime.recoveryMetadata.history],
  };
  const isPendingRetry = updated.status === "pending" && existing.status !== "pending";
  const isActiveStatus =
    updated.status === "assigned" || updated.status === "in-progress" || updated.status === "review";
  const hasWorkerEvidence = reportPersisted !== undefined || isActiveStatus || updated.status === "done";

  let runtimeState = runtime.runtimeState;
  let failureReason = runtime.failureReason;
  let agent = runtime.agent;
  let sessionId = runtime.sessionId;

  if (updated.status === "done") {
    runtimeState = "completed";
    failureReason = undefined;
  } else if (isPendingRetry) {
    const wasRuntimeRecovery = existing.status === "assigned" || existing.status === "in-progress";
    runtimeState = wasRuntimeRecovery ? "recoverable" : "starting";
    failureReason = wasRuntimeRecovery ? retryReason : undefined;
    agent = UNKNOWN_AGENT;
    sessionId = undefined;
  } else if (reportPersisted !== undefined || isActiveStatus) {
    runtimeState = "live";
    failureReason = undefined;
  }

  return {
    ...runtime,
    agent,
    sessionId,
    runtimeState,
    lastSeenAt: hasWorkerEvidence ? nowIso : runtime.lastSeenAt,
    leaseExpiresAt: hasWorkerEvidence
      ? new Date(now + DEFAULT_RUNTIME_LEASE_MS).toISOString()
      : runtime.leaseExpiresAt,
    failureReason,
    ...(reportPersisted !== undefined
      ? {
        recoveryMetadata,
      }
      : {
        recoveryMetadata,
      }),
  };
}

/**
 * Parse a worker report from inline JSON or @file syntax
 */
function isVerificationObj(v: unknown): v is WorkerReport["verification"] {
  return typeof v === "object" && v !== null && "commandsRun" in v && "interactiveChecks" in v;
}

function isTestsObj(t: unknown): t is WorkerReport["tests"] {
  return typeof t === "object" && t !== null && "added" in t;
}

export async function parseWorkerReport(
  reportValue: string,
): Promise<WorkerReport> {
  let reportContent: string;

  if (reportValue.startsWith("@")) {
    // Read from file
    const filePath = reportValue.slice(1);
    const { readText } = await import("../lib/fs.js");
    const content = await readText(filePath);
    if (content === undefined) {
      throw new MaestroError(`Report file not found: ${filePath}`, [
        `Check that the file exists: ${filePath}`,
        "Use absolute path or path relative to current directory",
      ]);
    }
    reportContent = content;
  } else {
    // Inline JSON
    reportContent = reportValue;
  }

  // Parse and validate the report
  let parsed: unknown;
  try {
    parsed = JSON.parse(reportContent);
  } catch {
    throw new MaestroError("Invalid JSON in worker report", [
      "Report must be valid JSON",
      "Use inline JSON or @file.json syntax",
    ]);
  }

  // Validate required fields
  if (typeof parsed !== "object" || parsed === null) {
    throw new MaestroError("Worker report must be a JSON object");
  }

  const reportObj = parsed as Record<string, unknown>;

  // Accept rich format (plan spec) with salientSummary
  if (typeof reportObj.salientSummary === "string") {
    const report: WorkerReport = {
      salientSummary: reportObj.salientSummary as string,
      whatWasImplemented: typeof reportObj.whatWasImplemented === "string" ? reportObj.whatWasImplemented : "",
      whatWasLeftUndone: typeof reportObj.whatWasLeftUndone === "string" ? reportObj.whatWasLeftUndone : "",
      verification: isVerificationObj(reportObj.verification)
        ? reportObj.verification as WorkerReport["verification"]
        : { commandsRun: [], interactiveChecks: [] },
      tests: isTestsObj(reportObj.tests)
        ? reportObj.tests as WorkerReport["tests"]
        : { added: [] },
      discoveredIssues: Array.isArray(reportObj.discoveredIssues)
        ? reportObj.discoveredIssues as WorkerReport["discoveredIssues"]
        : [],
    };
    return report;
  }

  // Accept legacy format with content field (backward compat)
  if (typeof reportObj.content === "string" && reportObj.content.length > 0) {
    const report: WorkerReport = {
      salientSummary: reportObj.content as string,
      whatWasImplemented: reportObj.content as string,
      whatWasLeftUndone: "",
      verification: { commandsRun: [], interactiveChecks: [] },
      tests: { added: [] },
      discoveredIssues: [],
    };
    return report;
  }

  throw new MaestroError("Worker report must have 'salientSummary' (preferred) or 'content' (legacy) field", [
    "Rich format: { salientSummary, whatWasImplemented, whatWasLeftUndone, verification, tests, discoveredIssues }",
    "Legacy format: { content: string }",
  ]);
}

/**
 * Persist a worker report to workers/{featureId}/report.json
 */
async function persistWorkerReport(
  baseDir: string,
  missionId: string,
  featureId: string,
  report: WorkerReport,
): Promise<string> {
  const workersDir = join(baseDir, MAESTRO_DIR, "missions", missionId, "workers", featureId);
  await ensureDir(workersDir);

  const reportPath = join(workersDir, "report.json");
  await writeJson(reportPath, report);

  return reportPath;
}

/** Get valid next states for a feature */
export function getValidFeatureNextStates(feature: Feature): readonly string[] {
  // Import dynamically to avoid circular dependencies - must be called from async context
  return getValidFeatureTransitions(feature.status);
}

// Re-export the transition function for direct use
import { getValidFeatureTransitions } from "../domain/mission-state.js";
