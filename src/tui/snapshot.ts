/**
 * Build a MissionControlSnapshot from existing stores.
 * Polls once -- no subscriptions, no event tailing.
 */
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../ports/checkpoint-store.port.js";
import type { Mission, Feature } from "../domain/mission-types.js";
import { generateMissionReport, type MissionReport } from "../usecases/mission-report.usecase.js";
import { getValidFeatureTransitions } from "../domain/mission-state.js";
import { deriveEvents } from "./events.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlWorkerPane,
  MissionControlMilestoneRow,
} from "./types.js";

export interface SnapshotDeps {
  missionStore: MissionStorePort;
  featureStore: FeatureStorePort;
  assertionStore: AssertionStorePort;
  checkpointStore: CheckpointStorePort;
}

/**
 * Build a complete snapshot for the mission control dashboard.
 * Throws if mission not found.
 */
export async function buildSnapshot(
  deps: SnapshotDeps,
  missionId: string,
): Promise<MissionControlSnapshot> {
  const report = await generateMissionReport(
    deps.missionStore,
    deps.featureStore,
    deps.assertionStore,
    missionId,
  );

  const features = await deps.featureStore.list(missionId);
  const assertions = await deps.assertionStore.list(missionId);
  const checkpoints = await deps.checkpointStore.list(missionId);

  const mission = report.mission;
  const now = Date.now();
  const startMs = new Date(mission.approvedAt ?? mission.createdAt).getTime();

  // Feature rows
  const featureRows: MissionControlFeatureRow[] = features.map((f) => ({
    id: f.id,
    title: f.title,
    status: f.status,
    milestoneId: f.milestoneId,
    workerType: f.workerType,
    hasReport: f.report !== undefined && f.report !== null,
  }));

  // Active feature: first assigned or in-progress
  const activeFeature = findActiveFeature(features, report);

  // Active worker
  const activeWorker = buildActiveWorker(features, startMs, now);

  // Progress log
  const progressLog = deriveEvents({
    mission,
    features,
    assertions,
    checkpoints,
    milestoneProgress: report.milestones,
  });

  // Milestone rows
  const milestones: MissionControlMilestoneRow[] = report.milestones.map((mp) => ({
    id: mp.milestoneId,
    title: mp.milestone.title,
    status: mp.status,
    order: mp.order,
  }));

  // Feature progress
  const doneCount = features.filter((f) => f.status === "done").length;
  const activeCount = features.filter(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  ).length;

  return {
    missionId: mission.id,
    missionTitle: mission.title,
    missionStatus: mission.status,
    effectiveStatus: report.effectiveMissionStatus,
    elapsedMs: now - startMs,
    featureProgress: { done: doneCount, total: features.length, active: activeCount },
    tokenCounters: null, // No telemetry infrastructure yet
    activeFeature,
    features: featureRows,
    activeWorker,
    progressLog,
    milestones,
    canPause: mission.status === "executing",
    canResume: mission.status === "paused",
  };
}

function findActiveFeature(
  features: readonly Feature[],
  report: MissionReport,
): MissionControlFeatureDetail | null {
  const active = features.find(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  ) ?? features.find((f) => f.status === "pending");

  if (!active) return null;

  const milestone = report.mission.milestones.find((m) => m.id === active.milestoneId);

  return {
    id: active.id,
    title: active.title,
    status: active.status,
    milestoneId: active.milestoneId,
    milestoneTitle: milestone?.title ?? active.milestoneId,
    workerType: active.workerType,
    description: active.description,
    preconditions: active.preconditions,
    expectedBehavior: active.expectedBehavior,
    verificationSteps: active.verificationSteps,
    dependsOn: active.dependsOn,
    fulfills: active.fulfills,
    validTransitions: [...getValidFeatureTransitions(active.status)],
  };
}

function buildActiveWorker(
  features: readonly Feature[],
  startMs: number,
  nowMs: number,
): MissionControlWorkerPane | null {
  const active = features.find(
    (f) => f.status === "assigned" || f.status === "in-progress",
  );

  if (!active) return null;

  const featureStartMs = new Date(active.updatedAt).getTime();

  return {
    featureId: active.id,
    featureTitle: active.title,
    workerType: active.workerType,
    status: active.status,
    elapsedMs: nowMs - featureStartMs,
    report: active.report ?? null,
  };
}
