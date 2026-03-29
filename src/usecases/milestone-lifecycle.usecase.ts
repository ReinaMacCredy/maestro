/**
 * Milestone lifecycle usecases
 * Implements milestone listing, status reporting, and seal functionality
 */
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type {
  Mission,
  Milestone,
  MilestoneStatus,
  Feature,
  Assertion,
  FeatureStatus,
  AssertionStatus,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import {
  assertMilestoneTransition,
  canTransitionMilestone,
  isTerminalAssertionStatus,
} from "../domain/mission-state.js";

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

/**
 * Compute the effective milestone status based on mission status and milestone order
 * This is a derivation - actual transitions happen via the state machine
 * If a milestone is already completed (derived from previous computation), it stays completed.
 */
function deriveMilestoneStatus(
  mission: Mission,
  milestone: Milestone,
  allMilestones: readonly Milestone[],
  sortedMilestoneStatuses: Map<string, MilestoneStatus>,
): MilestoneStatus {
  // Sort milestones by order
  const sorted = [...allMilestones].sort((a, b) => a.order - b.order);
  const currentIndex = sorted.findIndex((m) => m.id === milestone.id);

  // Check if this milestone is already completed in the status map
  const currentStatus = sortedMilestoneStatuses.get(milestone.id);
  if (currentStatus === "completed") {
    return "completed";
  }

  // Find the first non-completed milestone (the active one)
  let firstNonCompletedIndex = -1;
  for (let i = 0; i < sorted.length; i++) {
    const m = sorted[i]!;
    const status = sortedMilestoneStatuses.get(m.id);
    // A milestone is "completed" if it's marked completed
    if (status !== "completed") {
      firstNonCompletedIndex = i;
      break;
    }
  }

  // If all milestones are completed
  if (firstNonCompletedIndex === -1) {
    return "completed";
  }

  // If this milestone is before the first non-completed one, it's completed
  if (currentIndex < firstNonCompletedIndex) {
    return "completed";
  }

  // If this IS the first non-completed milestone, it gets the active status
  if (currentIndex === firstNonCompletedIndex) {
    switch (mission.status) {
      case "executing":
        return "executing";
      case "validating":
        return "validating";
      case "failed":
        return "failed";
      default:
        return "pending";
    }
  }

  // This milestone is after the active one - it should be pending
  return "pending";
}

/**
 * Calculate milestone progress metrics
 */
async function calculateMilestoneProgress(
  mission: Mission,
  milestone: Milestone,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  sortedMilestoneStatuses: Map<string, MilestoneStatus>,
): Promise<MilestoneProgress> {
  // Get all features and assertions for this milestone
  const features = await featureStore.list(mission.id, { milestoneId: milestone.id });
  const assertions = await assertionStore.listByMilestone(mission.id, milestone.id);

  const featureCount = features.length;
  const completedFeatures = features.filter((f) => f.status === "completed").length;
  const featureCompletionPct = featureCount > 0 ? Math.round((completedFeatures / featureCount) * 100) : 0;

  const assertionCount = assertions.length;
  const passedAssertions = assertions.filter((a) => a.status === "passed").length;
  const waivedAssertions = assertions.filter((a) => a.status === "waived").length;
  const terminalAssertions = assertions.filter((a) => isTerminalAssertionStatus(a.status)).length;
  const assertionCompletionPct = assertionCount > 0 ? Math.round((terminalAssertions / assertionCount) * 100) : 0;

  const waivedAssertionIds = assertions
    .filter((a) => a.status === "waived")
    .map((a) => a.id);

  const status = deriveMilestoneStatus(mission, milestone, mission.milestones, sortedMilestoneStatuses);

  return {
    milestoneId: milestone.id,
    milestone,
    status,
    featureCount,
    completedFeatures,
    featureCompletionPct,
    assertionCount,
    passedAssertions,
    waivedAssertions,
    terminalAssertions,
    assertionCompletionPct,
    waivedAssertionIds,
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

  // Sort milestones by order for sequential processing
  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);
  
  // Build a map of milestone statuses - start with completedMilestoneIds from mission
  // then compute remaining statuses in order
  const sortedMilestoneStatuses = new Map<string, MilestoneStatus>();
  const completedMilestoneIds = mission.completedMilestoneIds ?? [];
  
  // Initialize with completed milestones
  for (const m of sortedMilestones) {
    const status = completedMilestoneIds.includes(m.id) ? "completed" : "pending";
    sortedMilestoneStatuses.set(m.id, status);
  }

  // Calculate progress for each milestone in order
  const milestones: MilestoneProgress[] = [];
  for (const milestone of sortedMilestones) {
    const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore, sortedMilestoneStatuses);
    milestones.push(progress);
    sortedMilestoneStatuses.set(milestone.id, progress.status);
  }

  // Sort by milestone order (already sorted, but ensure)
  milestones.sort((a, b) => a.milestone.order - b.milestone.order);

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

  // Build status map for deriving correct milestone status
  const completedMilestoneIds = mission.completedMilestoneIds ?? [];
  const sortedMilestoneStatuses = new Map<string, MilestoneStatus>();
  for (const m of mission.milestones) {
    const status = completedMilestoneIds.includes(m.id) ? "completed" : "pending";
    sortedMilestoneStatuses.set(m.id, status);
  }

  const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore, sortedMilestoneStatuses);

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

  // Build status map for deriving correct milestone status
  const completedMilestoneIds = mission.completedMilestoneIds ?? [];
  const sortedMilestoneStatuses = new Map<string, MilestoneStatus>();
  for (const m of mission.milestones) {
    const status = completedMilestoneIds.includes(m.id) ? "completed" : "pending";
    sortedMilestoneStatuses.set(m.id, status);
  }

  // Get current progress to determine status
  const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore, sortedMilestoneStatuses);

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
    (a) => !isTerminalAssertionStatus(a.status),
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
    progress.status = "completed";
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
