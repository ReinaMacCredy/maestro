// Pure projection of raw store outputs into the MissionControlSnapshot read
// model. No I/O. Takes already-loaded inputs (see snapshot-loader.ts) and
// computes derived views.
import type { ConfigLayers } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import {
  type Mission,
  type Feature,
  type Milestone,
  type Assertion,
  type Checkpoint,
  type AgentReply,
  deriveMissionReport,
  type MissionReport,
  getValidFeatureTransitions,
} from "@/features/mission";
import { getMissionControlBackgroundMode } from "@/tui/shared/ui-config.js";
import type { DoctorCheck, StatusReport } from "@/infra/domain/status-types.js";
import { deriveEvents } from "./events.js";
import { buildConfigInspector } from "./config-inspector.js";
import type {
  AgentGridRow,
  DispatchQueueItem,
  EventStreamEntry,
  TaskBoardSnapshot,
  TimelineMilestoneEntry,
  InferredAgentStatus,
} from "./screen-types.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlMilestoneRow,
  MissionControlHomeAction,
  MissionControlEvent,
  MissionControlMemorySnapshot,
  BlockedByRef,
  TaskPreviewPane,
  MissionOverviewPane,
  DependencyMapRow,
} from "./types.js";
import { buildIgnoredProjectOverrideChecks } from "./environment-projection.js";
import { buildReplyInbox } from "./reply-projection.js";
import type { PrincipleEffectivenessRow, ReplyInboxEntry } from "./screen-types.js";
import type { AutopilotSnapshot } from "./autopilot-screen.js";

interface FeatureGraphEntry {
  readonly feature: Feature;
  readonly blockedBy: readonly Feature[];
  readonly unblocks: readonly Feature[];
}

export interface EnvironmentSummary {
  readonly status: StatusReport;
  readonly checks: readonly DoctorCheck[];
}

export interface SnapshotProjectionInput {
  readonly mission: Mission;
  readonly features: readonly Feature[];
  readonly assertions: readonly Assertion[];
  readonly checkpoints: readonly Checkpoint[];
  readonly env: EnvironmentSummary;
  readonly configLayers: ConfigLayers;
  readonly gitState: Awaited<ReturnType<GitPort["getState"]>>;
  readonly memorySnapshot: MissionControlMemorySnapshot | undefined;
  readonly taskBoard: TaskBoardSnapshot | undefined;
  readonly replies: readonly AgentReply[] | undefined;
  readonly principleEffectiveness: readonly PrincipleEffectivenessRow[] | undefined;
  readonly autopilot?: AutopilotSnapshot;
}

export interface HomeProjectionInput {
  readonly env: EnvironmentSummary;
  readonly configLayers: ConfigLayers;
  readonly gitState: Awaited<ReturnType<GitPort["getState"]>> | undefined;
  readonly memorySnapshot: MissionControlMemorySnapshot | undefined;
  readonly taskBoard: TaskBoardSnapshot | undefined;
  readonly replies: readonly AgentReply[] | undefined;
  readonly principleEffectiveness: readonly PrincipleEffectivenessRow[] | undefined;
  readonly cwd: string;
}

export function projectSnapshot(input: SnapshotProjectionInput): MissionControlSnapshot {
  const {
    mission,
    features,
    assertions,
    checkpoints,
    env,
    configLayers,
    gitState,
    memorySnapshot,
    taskBoard,
    replies,
    principleEffectiveness,
    autopilot,
  } = input;

  const report = deriveMissionReport(mission, features, assertions);
  const now = Date.now();
  const startMs = new Date(mission.approvedAt ?? mission.createdAt).getTime();
  const featureGraph = buildFeatureGraph(features);
  const taskPreviews = features.map((feature) =>
    buildTaskPreview(feature, report, featureGraph.get(feature.id))
  );
  const checks = [
    ...env.checks,
    ...buildIgnoredProjectOverrideChecks(configLayers.project),
  ];
  const backgroundMode = getMissionControlBackgroundMode(configLayers.effective);
  const taskPreviewById = new Map(taskPreviews.map((preview) => [preview.id, preview]));

  const featureRows: MissionControlFeatureRow[] = features.map((f) => {
    const preview = taskPreviewById.get(f.id);

    return {
      id: f.id,
      title: f.title,
      status: f.status,
      milestoneId: f.milestoneId,
      agentType: f.agentType,
      hasReport: f.report !== undefined && f.report !== null,
      blockedByIds: preview?.blockedBy?.map((item) => item.id) ?? [],
      blockedByLabel: buildBlockedByLabel(preview?.blockedBy ?? []),
    };
  });

  const activeFeature = findActiveFeature(taskPreviews);

  const progressLog = deriveEvents({
    mission,
    features,
    assertions,
    checkpoints,
    milestoneProgress: report.milestones,
  });

  const milestones: MissionControlMilestoneRow[] = report.milestones.map((mp) => ({
    id: mp.milestoneId,
    title: mp.milestone.title,
    status: mp.status,
    order: mp.order,
    kind: mp.milestone.kind ?? "work",
    profile: mp.milestone.profile ?? "custom",
  }));

  const doneCount = features.filter((f) => f.status === "done").length;
  const activeCount = features.filter(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  ).length;
  const blockedCount = features.filter((f) => f.status === "blocked").length;
  const queuedCount = features.filter((f) => f.status === "pending").length;
  const activeMilestone = milestones.find((m) => m.status === "executing" || m.status === "validating");
  const gateLabel = activeMilestone?.kind === "gate" ? activeMilestone.title : null;
  const gateBlocked = Boolean(activeMilestone && activeMilestone.kind === "gate"
    && features.some((f) => f.milestoneId === activeMilestone.id && f.status === "blocked"));
  const missionOverview = buildMissionOverview(
    mission,
    features,
    featureGraph,
    {
      doneCount,
      blockedCount,
      activeCount,
      currentMilestoneId: activeMilestone?.id ?? null,
      currentMilestone: activeMilestone?.title ?? null,
      gateLabel,
    },
  );
  const sessionSidebar = buildSessionSidebar(gitState);

  const replyInbox: readonly ReplyInboxEntry[] | undefined = replies
    ? buildReplyInbox(features, replies)
    : undefined;

  const agentGrid = buildAgentGrid(features);
  const missionMilestones = mission.milestones;
  const dispatchQueue = buildDispatchQueue(features, missionMilestones);

  const eventStream = buildEventStream(progressLog, replies ?? []);
  const timelineMilestones = buildTimelineMilestones(missionMilestones, features);

  return {
    mode: "mission",
    missionId: mission.id,
    missionTitle: mission.title,
    missionStatus: mission.status,
    effectiveStatus: report.effectiveMissionStatus,
    elapsedMs: now - startMs,
    featureProgress: { done: doneCount, total: features.length, active: activeCount },
    statusProgress: {
      completed: report.summary.totalCompletedFeatures,
      total: report.summary.totalFeatures,
      inFlight: activeCount,
      blocked: blockedCount,
      queued: queuedCount,
      completionPct: report.summary.overallFeaturePct,
      },
    tokenCounters: null,
    missionOverview,
    activeFeature,
    features: featureRows,
    taskPreviews,
    session: sessionSidebar,
    configSummary: {
      configSource: env.status.configSource,
      gitAvailable: env.status.gitAvailable,
      checks,
      missionDirectory: `.maestro/missions/${mission.id}`,
      backgroundMode,
    },
    configInspector: buildConfigInspector(configLayers, checks, features),
    progressLog,
    milestones,
    gateBlocked,
    gateLabel,
    canPause: mission.status === "executing",
    canResume: mission.status === "paused",
    memory: memorySnapshot,
    memoryStats: memorySnapshot?.stats ?? null,
    agentGrid,
    dispatchQueue,
    eventStream,
    taskBoard,
    timelineMilestones,
    replyInbox,
    principleEffectiveness,
    autopilot,
    home: null,
  };
}

export function projectHomeSnapshot(input: HomeProjectionInput): MissionControlSnapshot {
  const { env, configLayers, gitState, memorySnapshot, taskBoard, replies, principleEffectiveness, cwd } = input;
  const checks = [
    ...env.checks,
    ...buildIgnoredProjectOverrideChecks(configLayers.project),
  ];
  const { status } = env;
  const backgroundMode = getMissionControlBackgroundMode(configLayers.effective);

  const headline = status.gitAvailable
    ? "No missions yet"
    : "No project detected";

  const summary = status.gitAvailable
    ? "Initialize this repository, then create your first mission."
    : status.initialized
      ? "Global setup is ready. Open a project repository to start tracking missions here."
      : "Open a git repository to track missions, checkpoints, and launches here.";

  const actions = buildHomeActions(status, checks);

  const agentGrid = buildAgentGrid([]);
  const homeReplyInbox = replies ? buildReplyInbox([], replies) : undefined;
  const homeEventStream = buildEventStream([], replies ?? []);

  return {
    mode: "home",
    missionId: "home",
    missionTitle: headline,
    missionStatus: "approved",
    effectiveStatus: "approved",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 0, active: 0 },
    statusProgress: {
      completed: 0,
      total: 0,
      inFlight: 0,
      blocked: 0,
      queued: 0,
      completionPct: 0,
    },
    tokenCounters: null,
    missionOverview: null,
    activeFeature: null,
    features: [],
    taskPreviews: [],
    session: gitState ? buildSessionSidebar(gitState) : null,
    configSummary: {
      configSource: status.configSource,
      gitAvailable: status.gitAvailable,
      checks,
      missionDirectory: null,
      backgroundMode,
    },
    configInspector: buildConfigInspector(configLayers, checks, []),
    progressLog: [],
    milestones: [],
    gateBlocked: false,
    gateLabel: null,
    canPause: false,
    canResume: false,
    memory: memorySnapshot,
    memoryStats: memorySnapshot?.stats ?? null,
    agentGrid,
    dispatchQueue: [],
    eventStream: homeEventStream,
    taskBoard,
    timelineMilestones: [],
    replyInbox: homeReplyInbox,
    principleEffectiveness,
    home: {
      headline,
      summary,
      locationLabel: status.gitAvailable ? cwd : "Outside a git repository",
      checks,
      actions,
    },
  };
}

function findActiveFeature(taskPreviews: readonly TaskPreviewPane[]): MissionControlFeatureDetail | null {
  return taskPreviews.find(
    (feature) => feature.status === "assigned" || feature.status === "in-progress" || feature.status === "review",
  ) ?? taskPreviews.find((feature) => feature.status === "pending") ?? null;
}

function buildHomeActions(
  status: StatusReport,
  checks: readonly DoctorCheck[],
): readonly MissionControlHomeAction[] {
  const actions: MissionControlHomeAction[] = [];
  const projectConfig = checks.find((check: DoctorCheck) => check.name === "project-config");
  const globalConfig = checks.find((check: DoctorCheck) => check.name === "global-config");

  if (!status.gitAvailable) {
    actions.push({
      label: "Create a project repo",
      command: "git init",
      detail: "Initialize this folder as a git repository before project setup.",
    });
  }

  if (projectConfig?.status !== "ok") {
    actions.push({
      label: "Initialize this project",
      command: "maestro init",
      detail: "Create .maestro/config.yaml and enable project-local mission tracking.",
    });
  }

  if (globalConfig?.status !== "ok") {
    actions.push({
      label: "Initialize global config",
      command: "maestro init --global",
      detail: "Set shared defaults and global agent instructions.",
    });
  }

  actions.push({
    label: "Run environment checks",
    command: "maestro doctor",
    detail: "Verify git and config health before starting work.",
  });

  return actions;
}

function buildTaskPreview(
  active: Feature,
  report: MissionReport,
  graphEntry?: FeatureGraphEntry,
): TaskPreviewPane {
  const milestone = report.mission.milestones.find((m) => m.id === active.milestoneId);

  return {
    id: active.id,
    title: active.title,
    status: active.status,
    milestoneId: active.milestoneId,
    milestoneTitle: milestone?.title ?? active.milestoneId,
    agentType: active.agentType,
    description: active.description,
    preconditions: active.preconditions,
    expectedBehavior: active.expectedBehavior,
    verificationSteps: active.verificationSteps,
    dependsOn: active.dependsOn,
    blockedBy: (graphEntry?.blockedBy ?? []).map(toFeatureRef),
    unblocks: (graphEntry?.unblocks ?? []).map(toFeatureRef),
    fulfills: active.fulfills,
    validTransitions: [...getValidFeatureTransitions(active.status)],
  };
}

function buildFeatureGraph(features: readonly Feature[]): Map<string, FeatureGraphEntry> {
  const byId = new Map(features.map((feature) => [feature.id, feature]));
  const downstream = new Map<string, Feature[]>();

  for (const feature of features) {
    for (const depId of feature.dependsOn) {
      const bucket = downstream.get(depId) ?? [];
      bucket.push(feature);
      downstream.set(depId, bucket);
    }
  }

  return new Map(features.map((feature) => {
    const blockedBy = feature.dependsOn
      .map((depId) => byId.get(depId))
      .filter((dependency): dependency is Feature => dependency !== undefined && dependency.status !== "done");
    const unblocks = downstream.get(feature.id) ?? [];
    return [feature.id, { feature, blockedBy, unblocks }] satisfies [string, FeatureGraphEntry];
  }));
}

function buildBlockedByLabel(blockedBy: readonly BlockedByRef[]): string | undefined {
  if (blockedBy.length === 0) return undefined;
  return blockedBy.map((item) => item.id).join(",");
}

function buildMissionOverview(
  mission: Mission,
  features: readonly Feature[],
  featureGraph: ReadonlyMap<string, FeatureGraphEntry>,
  summary: {
    doneCount: number;
    blockedCount: number;
    activeCount: number;
    currentMilestoneId: string | null;
    currentMilestone: string | null;
    gateLabel: string | null;
  },
): MissionOverviewPane {
  return {
    missionLabel: `Mission: ${mission.title}`,
    statusLabel: mission.status,
    activeCount: summary.activeCount,
    doneCount: summary.doneCount,
    totalCount: features.length,
    blockedCount: summary.blockedCount,
    currentMilestone: summary.currentMilestone,
    gateLabel: summary.gateLabel,
    agentSummary: [],
    dependencyMap: buildMinimalDependencyMap(features, featureGraph, summary.currentMilestoneId),
  };
}

function buildMinimalDependencyMap(
  features: readonly Feature[],
  featureGraph: ReadonlyMap<string, FeatureGraphEntry>,
  currentMilestone: string | null,
): readonly DependencyMapRow[] {
  return features
    .map((feature) => {
      const graphEntry = featureGraph.get(feature.id);
      const dependents = graphEntry?.unblocks ?? [];
      const blockedChildren = dependents.filter((child) => child.status === "blocked");
      const prioritizedDependents = blockedChildren.length > 0 ? blockedChildren : dependents;
      const score = (feature.milestoneId === currentMilestone ? 100 : 0)
        + ((feature.status === "assigned" || feature.status === "in-progress") ? 50 : 0)
        + blockedChildren.length * 20
        + dependents.length * 10;
      return { feature, dependents, prioritizedDependents, score };
    })
    .filter((entry) => entry.dependents.length > 0)
    .sort((a, b) => b.score - a.score || a.feature.id.localeCompare(b.feature.id))
    .slice(0, 2)
    .map((entry) => ({
      root: toFeatureRef(entry.feature),
      primaryDependent: entry.prioritizedDependents[0] ? toFeatureRef(entry.prioritizedDependents[0]) : undefined,
      primaryDependentBlockedByCount: entry.prioritizedDependents[0]
        ? featureGraph.get(entry.prioritizedDependents[0].id)?.blockedBy.length ?? 0
        : undefined,
      hiddenDependentCount: Math.max(0, entry.dependents.length - 1),
    }));
}

function buildSessionSidebar(
  gitState: Awaited<ReturnType<GitPort["getState"]>>,
) {
  return {
    branch: gitState.branch,
    workingTreeClean: gitState.workingTreeClean,
    diffStat: gitState.diffStat,
    changedFiles: gitState.changedFiles,
    fileChanges: gitState.fileChanges ?? [],
  };
}

function toFeatureRef(feature: Feature): BlockedByRef {
  return {
    id: feature.id,
    title: feature.title,
    status: feature.status,
  };
}

const STALE_THRESHOLD_MS = 60 * 60 * 1000; // 1 hour

export function buildAgentGrid(
  features: readonly Feature[],
): readonly AgentGridRow[] {
  const byAgent = new Map<string, Feature[]>();
  for (const f of features) {
    const bucket = byAgent.get(f.agentType) ?? [];
    bucket.push(f);
    byAgent.set(f.agentType, bucket);
  }

  const rows: AgentGridRow[] = [];
  const agentTypes = new Set<string>(byAgent.keys());

  for (const agentType of agentTypes) {
    const agentFeatures = byAgent.get(agentType) ?? [];
    const active = agentFeatures.find(
      (f) => f.status === "assigned" || f.status === "in-progress",
    );
    const hasReview = agentFeatures.some((f) => f.status === "review");
    const allDone = agentFeatures.length > 0 && agentFeatures.every((f) => f.status === "done");
    const isStale = active !== undefined
      && (Date.now() - new Date(active.updatedAt).getTime()) > STALE_THRESHOLD_MS;

    let status: InferredAgentStatus;
    if (isStale) status = "stale";
    else if (active) status = "active";
    else if (hasReview) status = "waiting";
    else if (allDone) status = "completed";
    else status = "waiting";

    rows.push({
      agentType,
      status,
      activeFeatureId: active?.id,
      activeFeatureTitle: active?.title,
      lastActivityAt: active?.updatedAt,
      featureCount: agentFeatures.length,
      completedCount: agentFeatures.filter((f) => f.status === "done").length,
    });
  }

  const ORDER: Record<InferredAgentStatus, number> = { active: 0, waiting: 1, stale: 2, completed: 3 };
  rows.sort((a, b) => ORDER[a.status] - ORDER[b.status]);
  return rows;
}

export function buildDispatchQueue(
  features: readonly Feature[],
  milestones: readonly Milestone[],
): readonly DispatchQueueItem[] {
  const featureById = new Map(features.map((f) => [f.id, f]));
  const milestoneById = new Map(milestones.map((m) => [m.id, m]));

  const ready = features.filter((f) => {
    if (f.status !== "pending") return false;
    return f.dependsOn.every((depId) => featureById.get(depId)?.status === "done");
  });

  return ready
    .map((f) => {
      const milestone = milestoneById.get(f.milestoneId);
      return {
        featureId: f.id,
        featureTitle: f.title,
        milestoneId: f.milestoneId,
        milestoneTitle: milestone?.title ?? f.milestoneId,
        milestoneOrder: milestone?.order ?? 0,
        agentType: f.agentType,
      };
    })
    .sort((a, b) => a.milestoneOrder - b.milestoneOrder);
}

export function buildEventStream(
  progressLog: readonly MissionControlEvent[],
  replies: readonly AgentReply[] = [],
): readonly EventStreamEntry[] {
  const entries: EventStreamEntry[] = [];
  const baseMs = getEventStreamBaseMs(progressLog);

  for (const event of progressLog) {
    entries.push({
      timestamp: event.timestamp,
      relativeMs: event.relativeMs,
      kind: event.kind,
      title: event.title,
      detail: event.detail,
    });
  }

  for (const r of replies) {
    const replyMs = new Date(r.writtenAt).getTime();
    const title = r.outcome === "kicked-back"
      ? `${r.featureId} kicked back`
      : r.outcome === "abandoned"
        ? `${r.featureId} abandoned`
        : `${r.featureId} completed`;
    entries.push({
      timestamp: r.writtenAt,
      relativeMs: baseMs === undefined || Number.isNaN(replyMs)
        ? 0
        : replyMs - baseMs,
      kind: "reply",
      title,
      detail: r.notes,
    });
  }

  entries.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
  return entries.slice(0, 200);
}

function getEventStreamBaseMs(
  progressLog: readonly MissionControlEvent[],
): number | undefined {
  if (progressLog.length === 0) return undefined;

  let baseMs = Number.POSITIVE_INFINITY;
  for (const event of progressLog) {
    const eventMs = new Date(event.timestamp).getTime();
    if (Number.isNaN(eventMs)) continue;
    baseMs = Math.min(baseMs, eventMs - event.relativeMs);
  }

  return Number.isFinite(baseMs) ? baseMs : undefined;
}

export function buildTimelineMilestones(
  milestones: readonly Milestone[],
  features: readonly Feature[],
): readonly TimelineMilestoneEntry[] {
  return milestones.map((m) => {
    const milestoneFeatures = features.filter((f) => f.milestoneId === m.id);
    const doneCount = milestoneFeatures.filter((f) => f.status === "done").length;
    const totalCount = milestoneFeatures.length;
    return {
      id: m.id,
      title: m.title,
      order: m.order,
      kind: m.kind ?? "work",
      profile: m.profile ?? "custom",
      features: milestoneFeatures.map((f) => ({
        id: f.id,
        title: f.title,
        status: f.status,
      })),
      progressPct: totalCount > 0 ? Math.round((doneCount / totalCount) * 100) : 0,
    };
  });
}
