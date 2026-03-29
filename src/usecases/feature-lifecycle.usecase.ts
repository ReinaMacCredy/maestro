/**
 * Feature lifecycle usecases
 * Implements feature listing, updating, and worker report persistence
 */
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type {
  Feature,
  FeatureStatus,
  UpdateFeatureInput,
  WorkerReport,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import { assertFeatureTransition } from "../domain/mission-state.js";
import { writeJson, ensureDir } from "../lib/fs.js";
import { join } from "node:path";
import { MAESTRO_DIR } from "../domain/defaults.js";

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

  // Update the feature
  const updateInput: UpdateFeatureInput = {
    status: input.status,
    report: finalReport,
  };

  const updated = await featureStore.update(missionId, featureId, updateInput);
  if (!updated) {
    throw new MaestroError(`Failed to update feature ${featureId}`);
  }

  return { feature: updated, reportPersisted };
}

/**
 * Parse a worker report from inline JSON or @file syntax
 */
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

  if (typeof reportObj.content !== "string" || reportObj.content.length === 0) {
    throw new MaestroError("Worker report must have a non-empty 'content' field", [
      "Report format: { content: string, timestamp?: string, agent?: string }",
    ]);
  }

  const report: WorkerReport = {
    content: reportObj.content,
    timestamp: typeof reportObj.timestamp === "string"
      ? reportObj.timestamp
      : new Date().toISOString(),
    agent: typeof reportObj.agent === "string" ? reportObj.agent : undefined,
  };

  return report;
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
