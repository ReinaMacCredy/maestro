/**
 * Feature lifecycle usecases
 * Implements feature listing, updating, and worker report persistence
 */
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../../ports/mission-store.port.js";
import type {
  Feature,
  UpdateFeatureInput,
  WorkerReport,
} from "../../domain/mission-types.js";
import { MaestroError } from "@/shared/errors.js";
import { assertFeatureTransition } from "../../domain/mission-state.js";
import { writeJson, readJson, ensureDir } from "@/shared/lib/fs.js";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

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
 * Enforces legal state transitions and persists worker reports.
 *
 * BEHAVIOR CHANGE (v1.0.0): This no longer writes to the runtime store.
 * Runtime state is owned externally now that maestro does not spawn
 * workers. The only persistent effect is the feature/report update
 * (plus retry-log bookkeeping).
 */
export async function updateFeature(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
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

  return { feature: updated, reportPersisted, missionAutoStarted };
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
    const { readText } = await import("@/shared/lib/fs.js");
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
import { getValidFeatureTransitions } from "../../domain/mission-state.js";
