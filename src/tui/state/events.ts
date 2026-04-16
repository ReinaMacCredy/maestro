import type { Mission, Feature, Checkpoint, Assertion, MilestoneReportProgress } from "@/features/mission";
import type { MissionControlEvent } from "./types.js";

interface DeriveEventsInput {
  mission: Mission;
  features: readonly Feature[];
  assertions: readonly Assertion[];
  checkpoints: readonly Checkpoint[];
  milestoneProgress: readonly MilestoneReportProgress[];
}

/**
 * Derive events from entity timestamps.
 * Returns events sorted by timestamp descending (newest first).
 */
export function deriveEvents(input: DeriveEventsInput): readonly MissionControlEvent[] {
  const events: MissionControlEvent[] = [];
  const baseMs = getBaseTimestamp(input.mission);

  // Mission lifecycle events
  addMissionEvents(events, input.mission, baseMs);

  // Feature events
  for (const f of input.features) {
    addFeatureEvents(events, f, baseMs);
  }

  // Assertion events
  for (const a of input.assertions) {
    if (a.result !== "pending") {
      events.push({
        timestamp: a.updatedAt,
        relativeMs: toMs(a.updatedAt) - baseMs,
        kind: "assertion",
        title: `${a.id}: ${a.result}`,
        detail: a.evidence,
      });
    }
  }

  // Checkpoint events
  for (const cp of input.checkpoints) {
    events.push({
      timestamp: cp.timestamp,
      relativeMs: toMs(cp.timestamp) - baseMs,
      kind: "checkpoint",
      title: `Checkpoint saved: ${cp.id}`,
    });
  }

  // Sort descending by timestamp
  events.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
  return events;
}

function getBaseTimestamp(mission: Mission): number {
  return toMs(mission.approvedAt ?? mission.createdAt);
}

function toMs(iso: string): number {
  return new Date(iso).getTime();
}

function addMissionEvents(events: MissionControlEvent[], mission: Mission, baseMs: number): void {
  events.push({
    timestamp: mission.createdAt,
    relativeMs: 0,
    kind: "mission",
    title: "Mission created",
  });

  if (mission.approvedAt) {
    events.push({
      timestamp: mission.approvedAt,
      relativeMs: toMs(mission.approvedAt) - baseMs,
      kind: "mission",
      title: "Mission approved",
    });
  }

  if (mission.rejectedAt) {
    events.push({
      timestamp: mission.rejectedAt,
      relativeMs: toMs(mission.rejectedAt) - baseMs,
      kind: "mission",
      title: "Mission rejected",
    });
  }

  if (mission.completedAt) {
    events.push({
      timestamp: mission.completedAt,
      relativeMs: toMs(mission.completedAt) - baseMs,
      kind: "mission",
      title: "Mission completed",
    });
  }
}

function addFeatureEvents(events: MissionControlEvent[], feature: Feature, baseMs: number): void {
  // Creation
  events.push({
    timestamp: feature.createdAt,
    relativeMs: toMs(feature.createdAt) - baseMs,
    kind: "feature",
    title: `${feature.id} created`,
  });

  // Status change (if updatedAt differs from createdAt and status isn't pending)
  if (feature.updatedAt !== feature.createdAt && feature.status !== "pending") {
    events.push({
      timestamp: feature.updatedAt,
      relativeMs: toMs(feature.updatedAt) - baseMs,
      kind: "feature",
      title: `${feature.id} moved to ${feature.status}`,
    });
  }
}
