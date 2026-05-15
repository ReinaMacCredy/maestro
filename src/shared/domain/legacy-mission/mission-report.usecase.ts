/**
 * Mission reporting usecase
 * Provides enhanced mission status with milestone progress indicators and completion percentages
 */
import type { MissionStorePort } from "./ports/mission-store.port.js";
import type { FeatureStorePort } from "./ports/feature-store.port.js";
import type { AssertionStorePort } from "./ports/assertion-store.port.js";
import type {
  Mission,
  MissionStatus,
  Milestone,
  MilestoneStatus,
  Feature,
  Assertion,
} from "./types.js";
import { MaestroError } from "@/shared/errors.js";
import { isTerminalAssertionStatus } from "./state-machine.js";
import {
  deriveEffectiveMissionStatus,
  deriveSequentialMilestoneStatuses,
  type MilestoneActivitySnapshot,
} from "./progress-derivation.usecase.js";

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
  effectiveMissionStatus: MissionStatus;
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

function collectMilestoneData(
  milestone: Milestone,
  features: readonly Feature[],
  assertions: readonly Assertion[],
): {
  activity: MilestoneActivitySnapshot;
  progress: Omit<MilestoneReportProgress, "status">;
} {
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
    },
  };
}

function groupByMilestone<T extends { readonly milestoneId: string }>(
  items: readonly T[],
): ReadonlyMap<string, readonly T[]> {
  const grouped = new Map<string, T[]>();

  for (const item of items) {
    const bucket = grouped.get(item.milestoneId);
    if (bucket) {
      bucket.push(item);
    } else {
      grouped.set(item.milestoneId, [item]);
    }
  }

  return grouped;
}

export function deriveMissionReport(
  mission: Mission,
  features: readonly Feature[],
  assertions: readonly Assertion[],
): MissionReport {
  const featuresByMilestone = groupByMilestone(features);
  const assertionsByMilestone = groupByMilestone(assertions);
  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);
  const milestoneData = sortedMilestones.map((milestone) =>
    collectMilestoneData(
      milestone,
      featuresByMilestone.get(milestone.id) ?? [],
      assertionsByMilestone.get(milestone.id) ?? [],
    )
  );
  const milestoneStatuses = deriveSequentialMilestoneStatuses(
    mission,
    milestoneData.map((item) => item.activity),
  );
  const milestones: MilestoneReportProgress[] = milestoneData
    .map((item) => ({
      ...item.progress,
      status: milestoneStatuses.get(item.progress.milestoneId) ?? "pending",
    }))
    .sort((a, b) => a.order - b.order);

  const totalFeatures = milestones.reduce((sum, m) => sum + m.featureCount, 0);
  const totalCompletedFeatures = milestones.reduce((sum, m) => sum + m.completedFeatures, 0);
  const overallFeaturePct = totalFeatures > 0 ? Math.round((totalCompletedFeatures / totalFeatures) * 100) : 0;

  const totalAssertions = milestones.reduce((sum, m) => sum + m.assertionCount, 0);
  const totalTerminalAssertions = milestones.reduce((sum, m) => sum + m.terminalAssertions, 0);
  const overallAssertionPct = totalAssertions > 0 ? Math.round((totalTerminalAssertions / totalAssertions) * 100) : 0;
  const totalWaivedAssertions = milestones.reduce((sum, m) => sum + m.waivedAssertions, 0);
  const effectiveMissionStatus = deriveEffectiveMissionStatus(mission, milestoneStatuses);

  return {
    mission,
    effectiveMissionStatus,
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

  const [features, assertions] = await Promise.all([
    featureStore.list(missionId),
    assertionStore.list(missionId),
  ]);

  return deriveMissionReport(mission, features, assertions);
}
