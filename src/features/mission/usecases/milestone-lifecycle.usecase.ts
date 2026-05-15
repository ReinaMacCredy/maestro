/**
 * Milestone lifecycle usecases
 * Implements milestone listing, status reporting, and seal functionality
 */
import type { MissionStorePort } from "@/shared/domain/legacy-mission";
import type { FeatureStorePort } from "@/shared/domain/legacy-mission";
import type { AssertionStorePort } from "@/shared/domain/legacy-mission";
import type {
  Mission,
  Milestone,
  MilestoneStatus,
  Feature,
  Assertion,
  FeatureStatus,
  AssertionResult,
} from "@/shared/domain/legacy-mission";
import { MaestroError } from "@/shared/errors.js";
import {
  canTransitionMilestone,
  isTerminalAssertionStatus,
} from "@/shared/domain/legacy-mission";
import {
  deriveSequentialMilestoneStatuses,
  type MilestoneActivitySnapshot,
} from "./progress-derivation.usecase.js";

/** Progress information for a milestone */
export interface MilestoneProgress {
  milestoneId: string;
  milestone: Milestone;
  status: MilestoneStatus;
  featureCount: number;
  completedFeatures: number;
  featureCompletionPct: number;
  assertionCount: number;
  passedAssertions: number;
  waivedAssertions: number;
  terminalAssertions: number;
  assertionCompletionPct: number;
  waivedAssertionIds: string[];
}

/** Result of listing milestones */
export interface ListMilestonesResult {
  mission: Mission;
  milestones: readonly MilestoneProgress[];
}

/** Result of getting milestone status */
export interface GetMilestoneStatusResult {
  mission: Mission;
  milestone: Milestone;
  progress: MilestoneProgress;
}

/** Result of sealing a milestone */
export interface SealMilestoneResult {
  mission: Mission;
  milestone: Milestone;
  progress: MilestoneProgress;
  sealed: boolean;
  autoTransitioned: boolean;
  blockingAssertionIds: string[];
}

async function collectMilestoneProgress(
  mission: Mission,
  milestone: Milestone,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
): Promise<{
  activity: MilestoneActivitySnapshot;
  progress: Omit<MilestoneProgress, "status">;
}> {
  // Get all features and assertions for this milestone
  const features = await featureStore.list(mission.id, { milestoneId: milestone.id });
  const assertions = await assertionStore.listByMilestone(mission.id, milestone.id);

  const featureCount = features.length;
  const completedFeatures = features.filter((f) => f.status === "done").length;
  const featureCompletionPct = featureCount > 0 ? Math.round((completedFeatures / featureCount) * 100) : 0;

  const assertionCount = assertions.length;
  const passedAssertions = assertions.filter((a) => a.result === "passed").length;
  const waivedAssertions = assertions.filter((a) => a.result === "waived").length;
  const terminalAssertions = assertions.filter((a) => isTerminalAssertionStatus(a.result)).length;
  const assertionCompletionPct = assertionCount > 0 ? Math.round((terminalAssertions / assertionCount) * 100) : 0;

  const waivedAssertionIds = assertions
    .filter((a) => a.result === "waived")
    .map((a) => a.id);

  return {
    activity: {
      milestoneId: milestone.id,
      order: milestone.order,
      hasStartedFeatures: features.some((feature) => feature.status !== "pending"),
      allFeaturesCompleted: featureCount > 0 && completedFeatures === featureCount,
    },
    progress: {
      milestoneId: milestone.id,
      milestone,
      featureCount,
      completedFeatures,
      featureCompletionPct,
      assertionCount,
      passedAssertions,
      waivedAssertions,
      terminalAssertions,
      assertionCompletionPct,
      waivedAssertionIds,
    },
  };
}

/**
 * List all milestones with progress for a mission
 */
export async function listMilestones(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
): Promise<ListMilestonesResult> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);
  const milestoneData = await Promise.all(
    sortedMilestones.map((milestone) =>
      collectMilestoneProgress(mission, milestone, featureStore, assertionStore),
    ),
  );
  const milestoneStatuses = deriveSequentialMilestoneStatuses(
    mission,
    milestoneData.map((item) => item.activity),
  );
  const milestones: MilestoneProgress[] = milestoneData
    .map((item) => ({
      ...item.progress,
      status: milestoneStatuses.get(item.progress.milestoneId) ?? "pending",
    }))
    .sort((a, b) => a.milestone.order - b.milestone.order);

  return { mission, milestones };
}

/**
 * Get detailed status for a specific milestone
 */
export async function getMilestoneStatus(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
  milestoneId: string,
): Promise<GetMilestoneStatusResult> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  const milestone = mission.milestones.find((m) => m.id === milestoneId);
  if (!milestone) {
    throw new MaestroError(`Milestone ${milestoneId} not found in mission ${missionId}`, [
      `List milestones: maestro milestone list --mission ${missionId}`,
      `Available milestones: ${mission.milestones.map((m) => m.id).join(", ")}`,
    ]);
  }

  const milestoneData = await Promise.all(
    mission.milestones.map((item) =>
      collectMilestoneProgress(mission, item, featureStore, assertionStore),
    ),
  );
  const milestoneStatuses = deriveSequentialMilestoneStatuses(
    mission,
    milestoneData.map((item) => item.activity),
  );
  const progressData = milestoneData.find((item) => item.progress.milestoneId === milestoneId);
  if (!progressData) {
    throw new MaestroError(`Failed to calculate progress for milestone ${milestoneId}`);
  }
  const progress: MilestoneProgress = {
    ...progressData.progress,
    status: milestoneStatuses.get(milestoneId) ?? "pending",
  };

  return { mission, milestone, progress };
}

/**
 * Seal a milestone - auto-transition executing milestones to validating, check all assertions are terminal
 * Returns success if all assertions are passed/waived, or lists blocking assertions
 */
export async function sealMilestone(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
  milestoneId: string,
): Promise<SealMilestoneResult> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  const milestone = mission.milestones.find((m) => m.id === milestoneId);
  if (!milestone) {
    throw new MaestroError(`Milestone ${milestoneId} not found in mission ${missionId}`, [
      `List milestones: maestro milestone list --mission ${missionId}`,
      `Available milestones: ${mission.milestones.map((m) => m.id).join(", ")}`,
    ]);
  }

  const milestoneData = await Promise.all(
    mission.milestones.map((item) =>
      collectMilestoneProgress(mission, item, featureStore, assertionStore),
    ),
  );
  const milestoneStatuses = deriveSequentialMilestoneStatuses(
    mission,
    milestoneData.map((item) => item.activity),
  );
  const progressData = milestoneData.find((item) => item.progress.milestoneId === milestoneId);
  if (!progressData) {
    throw new MaestroError(`Failed to calculate progress for milestone ${milestoneId}`);
  }
  const completedMilestoneIds = mission.completedMilestoneIds ?? [];
  const progress: MilestoneProgress = {
    ...progressData.progress,
    status: milestoneStatuses.get(milestoneId) ?? "pending",
  };

  // Auto-transition from executing to validating if needed
  let autoTransitioned = false;
  if (progress.status === "executing") {
    if (canTransitionMilestone("executing", "validating")) {
      autoTransitioned = true;
      progress.status = "validating";
    }
  }

  // Get all assertions for this milestone
  const assertions = await assertionStore.listByMilestone(mission.id, milestoneId);

  // Find non-terminal assertions (those that block sealing)
  const nonTerminalAssertions = assertions.filter(
    (a) => !isTerminalAssertionStatus(a.result),
  );

  const blockingAssertionIds = nonTerminalAssertions.map((a) => a.id);

  // Can only seal if all assertions are terminal (passed or waived)
  const canSeal = blockingAssertionIds.length === 0;

  // If sealing succeeds, persist the milestone as completed
  if (canSeal && !completedMilestoneIds.includes(milestoneId)) {
    const updatedCompletedIds = [...completedMilestoneIds, milestoneId];
    await missionStore.update(missionId, {
      completedMilestoneIds: updatedCompletedIds,
    });
    progress.status = "sealed";
  }

  return {
    mission,
    milestone,
    progress,
    sealed: canSeal,
    autoTransitioned,
    blockingAssertionIds,
  };
}
