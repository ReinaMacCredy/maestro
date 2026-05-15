import type {
  Mission,
  MissionStatus,
  MilestoneStatus,
} from "@/shared/domain/legacy-mission";

export interface MilestoneActivitySnapshot {
  readonly milestoneId: string;
  readonly order: number;
  readonly hasStartedFeatures: boolean;
  readonly allFeaturesCompleted: boolean;
}

export function deriveSequentialMilestoneStatuses(
  mission: Mission,
  activities: readonly MilestoneActivitySnapshot[],
): Map<string, MilestoneStatus> {
  const completedMilestoneIds = new Set(mission.completedMilestoneIds ?? []);
  const sortedActivities = [...activities].sort((a, b) => a.order - b.order);
  const statuses = new Map<string, MilestoneStatus>();
  let activeAssigned = false;

  for (const activity of sortedActivities) {
    if (completedMilestoneIds.has(activity.milestoneId)) {
      statuses.set(activity.milestoneId, "sealed");
      continue;
    }

    if (activeAssigned) {
      statuses.set(activity.milestoneId, "pending");
      continue;
    }

    statuses.set(
      activity.milestoneId,
      deriveActiveMilestoneStatus(mission.status, activity),
    );
    activeAssigned = true;
  }

  return statuses;
}

export function deriveEffectiveMissionStatus(
  mission: Mission,
  milestoneStatuses: ReadonlyMap<string, MilestoneStatus>,
): MissionStatus {
  switch (mission.status) {
    case "draft":
    case "rejected":
    case "completed":
    case "failed":
      return mission.status;
  }

  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);
  if (
    sortedMilestones.length > 0 &&
    sortedMilestones.every((milestone) => milestoneStatuses.get(milestone.id) === "sealed")
  ) {
    return "completed";
  }

  for (const milestone of sortedMilestones) {
    const status = milestoneStatuses.get(milestone.id);
    if (!status || status === "sealed") {
      continue;
    }
    if (status !== "pending") {
      return status;
    }
    break;
  }

  return mission.status;
}

export function getCurrentMilestoneId(
  mission: Mission,
  milestoneStatuses: ReadonlyMap<string, MilestoneStatus>,
): string {
  const sortedMilestones = [...mission.milestones].sort((a, b) => a.order - b.order);

  for (const milestone of sortedMilestones) {
    if (milestoneStatuses.get(milestone.id) !== "sealed") {
      return milestone.id;
    }
  }

  return sortedMilestones[0]?.id ?? "";
}

function deriveActiveMilestoneStatus(
  missionStatus: MissionStatus,
  activity: MilestoneActivitySnapshot,
): MilestoneStatus {
  if (activity.hasStartedFeatures) {
    return activity.allFeaturesCompleted ? "validating" : "executing";
  }

  switch (missionStatus) {
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
