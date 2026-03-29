/**
 * Mission reporting usecase
 * Provides enhanced mission status with milestone progress indicators and completion percentages
 */
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type {
  Mission,
  Milestone,
  Feature,
  Assertion,
  MilestoneStatus,
  FeatureStatus,
  AssertionStatus,
} from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import { isTerminalAssertionStatus } from "../domain/mission-state.js";

/** Progress information for a milestone in the report */
export interface MilestoneReportProgress {
  milestoneId: string;
  milestone: Milestone;
  status: MilestoneStatus;
  order: number;
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

/** Enhanced mission report with progress indicators */
export interface MissionReport {
  mission: Mission;
  milestones: readonly MilestoneReportProgress[];
  summary: {
    totalFeatures: number;
    totalCompletedFeatures: number;
    overallFeaturePct: number;
    totalAssertions: number;
    totalTerminalAssertions: number;
    overallAssertionPct: number;
    totalWaivedAssertions: number;
  };
}

/**
 * Derive milestone status based on mission status and milestone order
 */
function deriveMilestoneStatus(
  mission: Mission,
  milestone: Milestone,
  allMilestones: readonly Milestone[],
): MilestoneStatus {
  const sorted = [...allMilestones].sort((a, b) => a.order - b.order);
  const currentIndex = sorted.findIndex((m) => m.id === milestone.id);

  switch (mission.status) {
    case "draft":
    case "approved":
      return "pending";
    case "executing":
      for (let i = 0; i < sorted.length; i++) {
        if (i < currentIndex) {
          continue;
        } else if (i === currentIndex) {
          return "executing";
        } else {
          return "pending";
        }
      }
      return "completed";
    case "validating":
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
 * Calculate progress for a single milestone
 */
async function calculateMilestoneProgress(
  mission: Mission,
  milestone: Milestone,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
): Promise<MilestoneReportProgress> {
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
    order: milestone.order,
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
 * Generate an enhanced mission report with progress indicators
 * Used by `maestro mission show <id>` to display milestone progress
 */
export async function generateMissionReport(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  missionId: string,
): Promise<MissionReport> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Calculate progress for each milestone
  const milestones: MilestoneReportProgress[] = [];
  for (const milestone of mission.milestones) {
    const progress = await calculateMilestoneProgress(mission, milestone, featureStore, assertionStore);
    milestones.push(progress);
  }

  // Sort by milestone order
  milestones.sort((a, b) => a.order - b.order);

  // Calculate overall summary
  const totalFeatures = milestones.reduce((sum, m) => sum + m.featureCount, 0);
  const totalCompletedFeatures = milestones.reduce((sum, m) => sum + m.completedFeatures, 0);
  const overallFeaturePct = totalFeatures > 0 ? Math.round((totalCompletedFeatures / totalFeatures) * 100) : 0;

  const totalAssertions = milestones.reduce((sum, m) => sum + m.assertionCount, 0);
  const totalTerminalAssertions = milestones.reduce((sum, m) => sum + m.terminalAssertions, 0);
  const overallAssertionPct = totalAssertions > 0 ? Math.round((totalTerminalAssertions / totalAssertions) * 100) : 0;
  const totalWaivedAssertions = milestones.reduce((sum, m) => sum + m.waivedAssertions, 0);

  return {
    mission,
    milestones,
    summary: {
      totalFeatures,
      totalCompletedFeatures,
      overallFeaturePct,
      totalAssertions,
      totalTerminalAssertions,
      overallAssertionPct,
      totalWaivedAssertions,
    },
  };
}
