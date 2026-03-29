/**
 * Checkpoint lifecycle usecases
 * Implements checkpoint save, list, and load functionality
 * Captures timestamped snapshots of mission execution state
 */
import type { CheckpointStorePort } from "../ports/checkpoint-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type {
  Checkpoint,
  FeatureStatus,
  AssertionStatus,
  Mission,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";

/** Result of saving a checkpoint */
export interface SaveCheckpointResult {
  checkpoint: Checkpoint;
}

/** Result of listing checkpoints */
export interface ListCheckpointsResult {
  mission: Mission;
  checkpoints: readonly Checkpoint[];
}

/** Result of loading the latest checkpoint */
export interface LoadCheckpointResult {
  checkpoint: Checkpoint;
  warning: string;
}

/**
 * Save a checkpoint for a mission
 * Captures current milestone, feature states, and assertion states
 */
export async function saveCheckpoint(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  checkpointStore: CheckpointStorePort,
  missionId: string,
): Promise<SaveCheckpointResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get all features and their statuses
  const features = await featureStore.list(missionId);
  const featureStates: Record<string, FeatureStatus> = {};
  for (const feature of features) {
    featureStates[feature.id] = feature.status;
  }

  // Get all assertions and their statuses
  const assertions = await assertionStore.list(missionId);
  const assertionStates: Record<string, AssertionStatus> = {};
  for (const assertion of assertions) {
    assertionStates[assertion.id] = assertion.status;
  }

  // Determine current milestone (first non-completed milestone, or first milestone)
  let currentMilestoneId = mission.milestones[0]?.id ?? "";
  for (const milestone of mission.milestones) {
    const milestoneFeatures = features.filter((f) => f.milestoneId === milestone.id);
    const hasInProgress = milestoneFeatures.some((f) => f.status === "in_progress");
    const isValidating = mission.status === "validating";
    const isExecuting = mission.status === "executing";
    
    if (hasInProgress || (isExecuting || isValidating) && milestoneFeatures.some((f) => f.status !== "completed")) {
      currentMilestoneId = milestone.id;
      break;
    }
  }

  // Create and save the checkpoint
  const checkpoint = await checkpointStore.save(missionId, {
    missionId,
    milestoneId: currentMilestoneId,
    timestamp: new Date().toISOString(),
    featureStates,
    assertionStates,
  });

  return { checkpoint };
}

/**
 * List all checkpoints for a mission
 * Returns checkpoints sorted newest-first
 */
export async function listCheckpoints(
  missionStore: MissionStorePort,
  checkpointStore: CheckpointStorePort,
  missionId: string,
): Promise<ListCheckpointsResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // List checkpoints (already sorted newest-first by adapter)
  const checkpoints = await checkpointStore.list(missionId);

  return { mission, checkpoints };
}

/**
 * Load the latest checkpoint for a mission
 * Returns the most recent checkpoint with a metadata-only restore warning
 */
export async function loadCheckpoint(
  missionStore: MissionStorePort,
  checkpointStore: CheckpointStorePort,
  missionId: string,
): Promise<LoadCheckpointResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get the latest checkpoint
  const checkpoint = await checkpointStore.load(missionId);
  if (!checkpoint) {
    throw new MaestroError(`No checkpoints found for mission ${missionId}`, [
      "Save a checkpoint first: maestro checkpoint save --mission <id>",
      "List checkpoints: maestro checkpoint list --mission <id>",
    ]);
  }

  // Include warning about metadata-only restore
  const warning =
    "WARNING: Checkpoints restore metadata only (mission state, feature statuses, assertion results). " +
    "Filesystem changes and git state are NOT restored by checkpoint load. " +
    "To restore code state, checkout the corresponding git commit if available.";

  return { checkpoint, warning };
}
