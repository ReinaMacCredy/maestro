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
  AssertionResult,
  Mission,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import {
  deriveSequentialMilestoneStatuses,
  getCurrentMilestoneId,
  type MilestoneActivitySnapshot,
} from "./progress-derivation.usecase.js";

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
  restored: {
    featureCount: number;
    assertionCount: number;
  };
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
  const featureStatuses: Record<string, FeatureStatus> = {};
  for (const feature of features) {
    featureStatuses[feature.id] = feature.status;
  }

  // Get all assertions and their results
  const assertions = await assertionStore.list(missionId);
  const assertionResults: Record<string, AssertionResult> = {};
  for (const assertion of assertions) {
    assertionResults[assertion.id] = assertion.result;
  }

  const milestoneActivities: MilestoneActivitySnapshot[] = mission.milestones.map((milestone) => {
    const milestoneFeatures = features.filter((feature) => feature.milestoneId === milestone.id);
    return {
      milestoneId: milestone.id,
      order: milestone.order,
      hasStartedFeatures: milestoneFeatures.some((feature) => feature.status !== "pending"),
      allFeaturesCompleted:
        milestoneFeatures.length > 0 &&
        milestoneFeatures.every((feature) => feature.status === "completed"),
    };
  });
  const milestoneStatuses = deriveSequentialMilestoneStatuses(mission, milestoneActivities);
  const currentMilestoneId = getCurrentMilestoneId(mission, milestoneStatuses);

  // Create and save the checkpoint
  const checkpoint = await checkpointStore.save(missionId, {
    missionId,
    currentMilestoneId,
    timestamp: new Date().toISOString(),
    featureStatuses,
    assertionResults,
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
 * Returns the most recent checkpoint metadata and clearly reports restore scope
 */
export async function loadCheckpoint(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
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

  const features = await featureStore.list(missionId);
  let restoredFeatureCount = 0;
  for (const feature of features) {
    const checkpointStatus = checkpoint.featureStatuses[feature.id];
    if (checkpointStatus !== undefined && checkpointStatus !== feature.status) {
      const updated = await featureStore.update(missionId, feature.id, { status: checkpointStatus });
      if (!updated) {
        throw new MaestroError(`Failed to restore feature ${feature.id} from checkpoint ${checkpoint.id}`);
      }
      restoredFeatureCount += 1;
    }
  }

  const assertions = await assertionStore.list(missionId);
  let restoredAssertionCount = 0;
  for (const assertion of assertions) {
    const checkpointResult = checkpoint.assertionResults[assertion.id];
    if (checkpointResult !== undefined && checkpointResult !== assertion.result) {
      const updated = await assertionStore.update(missionId, assertion.id, { result: checkpointResult });
      if (!updated) {
        throw new MaestroError(`Failed to restore assertion ${assertion.id} from checkpoint ${checkpoint.id}`);
      }
      restoredAssertionCount += 1;
    }
  }

  return {
    checkpoint,
    restored: {
      featureCount: restoredFeatureCount,
      assertionCount: restoredAssertionCount,
    },
  };
}
