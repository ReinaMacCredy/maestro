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
 */
function deriveMilestoneStatus(
  mission: Mission,
  milestone: Milestone,
  allMilestones: readonly Milestone[],
): MilestoneStatus {
  // Sort milestones by order
  const sorted = [...allMilestones].sort((a, b) => a.order - b.order);
  const currentIndex = sorted.findIndex((m) => m.id === milestone.id);

  // Derive based on mission status and position
  switch (mission.status) {
    case "draft":
    case "approved":
      return "pending";
    case "executing":
      // Find the first non-completed milestone
      for (let i = 0; i < sorted.length; i++) {
        if (i < currentIndex) {
          // Previous milestones should be completed
          continue;
        } else if (i === currentIndex) {
          return "executing";
        } else {
          return "pending";
        }
      }
      return "completed";
    case "validating":
      // During validation, current milestone is validating
      for (let i = 0; i < sorted.length; i++) {
        if (i < currentIndex) {
          continue;
        } else if (i === currentIndex) {
          return "validating";
        } else {
          return "pending";
        }
      }
      return "completed";
    case "completed":
      return "completed";
    case "failed":
      // If mission failed, the current milestone failed
      for (let i = 0; i < sorted.length; i++) {
        if (i === currentIndex) {
          return currentIndex < sorted.length - 1 ? "failed" : "completed";
        }
      }
      return "completed";
    case "rejected":
      return "pending";
    default:
      return "pending";
  }
}

/**
 * Calculate milestone progress metrics
 */
async function calculateMilestoneProgress(
  mission: Mission,
  milestone: Milestone,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
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

  const status = deriveMilestoneStatus(mission, milestone, mission.milestones);

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

  // Calculate progress for each milestone
  const milestones: MilestoneProgress[] = [];
  for (const milestone of mission.milestones) {
    const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore);
    milestones.push(progress);
  }

  // Sort by milestone order
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

  const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore);

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

  // Get current progress to determine status
  const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore);

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

  return {
    mission,
    milestone,
    progress,
    sealed: canSeal,
    autoTransitioned,
    blockingAssertionIds,
  };
}
