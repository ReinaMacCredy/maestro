// Build a MissionControlSnapshot from existing stores. Polls once per call;
// no subscriptions, no event tailing.
import { basename } from "node:path";
import { cached, setCachedEntry, type CacheEntry } from "@/tui/lib/snapshot-poll-cache.js";
import { MaestroError } from "@/shared/errors.js";
import type {
  MissionStorePort,
  FeatureStorePort,
  AssertionStorePort,
  CheckpointStorePort,
} from "@/features/mission";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { CorrectionStorePort, LearningStorePort } from "@/features/memory";
import { buildMemoryStats } from "@/features/memory";
import type { RatchetStorePort } from "@/features/ratchet";
import type { ProjectGraphStorePort } from "@/features/graph";
import type { HandoffStorePort, UkiHandoff } from "@/features/handoff";
import { TASK_STATUSES, type TaskQueryPort, type TaskStatus } from "@/features/task";
import type { ReplyStorePort, WorkerReply, ReplyOutcome } from "@/features/reply";
import { ingestReply } from "@/features/reply";
import type {
  Principle,
  PrincipleOutcomeRecord,
  PrincipleStorePort,
} from "@/features/mission";
import {
  buildPrincipleEffectiveness,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "@/features/mission";
import { recommendWorkerFit } from "@/features/agent";
import {
  type Mission,
  type Feature,
  type Milestone,
  deriveMissionReport,
  type MissionReport,
  getValidFeatureTransitions,
} from "@/features/mission";
import { getMissionControlBackgroundMode, listIgnoredProjectConfigKeys } from "@/shared/domain/ui-config.js";
import type { DoctorCheck, StatusReport } from "@/infra/domain/status-types.js";
import { getGraphContext } from "@/features/graph";
import { deriveEvents } from "./events.js";
import { buildConfigInspector } from "./config-inspector.js";
import type {
  AgentGridRow,
  DispatchQueueItem,
  EventStreamEntry,
  PrincipleEffectivenessRow,
  ReplyInboxEntry,
  TaskBoardSnapshot,
  TaskBoardItem,
  TimelineMilestoneEntry,
  InferredAgentStatus,
} from "./screen-types.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlMilestoneRow,
  MissionControlHomeAction,
  MissionControlHomeHandoff,
  MissionControlEvent,
  BlockedByRef,
  TaskPreviewPane,
  MissionOverviewPane,
  DependencyMapRow,
  MissionControlMemorySnapshot,
} from "./types.js";

export interface SnapshotDeps {
  missionStore: MissionStorePort;
  featureStore: FeatureStorePort;
  assertionStore: AssertionStorePort;
  checkpointStore: CheckpointStorePort;
  config: ConfigPort;
  git: GitPort;
  correctionStore?: CorrectionStorePort;
  learningStore?: LearningStorePort;
  ratchetStore?: RatchetStorePort;
  projectGraphStore?: ProjectGraphStorePort;
  handoffStore?: HandoffStorePort;
  taskStore?: TaskQueryPort;
  replyStore?: ReplyStorePort;
  principleStore?: PrincipleStorePort;
  cwd: string;
}

export interface HomeSnapshotDeps {
  config: ConfigPort;
  git: GitPort;
  correctionStore?: CorrectionStorePort;
  learningStore?: LearningStorePort;
  ratchetStore?: RatchetStorePort;
  projectGraphStore?: ProjectGraphStorePort;
  handoffStore?: HandoffStorePort;
  taskStore?: TaskQueryPort;
  replyStore?: ReplyStorePort;
  principleStore?: PrincipleStorePort;
  cwd: string;
}

export interface SnapshotBuildOptions {
  includeTaskBoard?: boolean;
  includeReplies?: boolean;
}

interface FeatureGraphEntry {
  readonly feature: Feature;
  readonly blockedBy: readonly Feature[];
  readonly unblocks: readonly Feature[];
}

/**
 * Build a complete snapshot for the mission control dashboard.
 * Throws if mission not found.
 */
export async function buildSnapshot(
  deps: SnapshotDeps,
  missionId: string,
  options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const taskBoardPromise = options.includeTaskBoard === true
    ? buildTaskBoard(deps.taskStore)
    : Promise.resolve(undefined);

  // Ingest replies FIRST when requested, so the features list below reflects
  // post-ingest state (advanced/kicked-back). Without this the inbox appears
  // stale for one poll cycle.
  const ingest = options.includeReplies === true
    ? await loadAndIngestReplies(deps, missionId)
    : { replies: undefined as readonly WorkerReply[] | undefined, outcomesCache: undefined };
  const replies = ingest.replies;

  const [
    mission,
    features,
    assertions,
    checkpoints,
    env,
    configLayers,
    gitState,
    memorySnapshot,
    pendingHandoffs,
    taskBoard,
  ] = await Promise.all([
    deps.missionStore.get(missionId),
    deps.featureStore.list(missionId),
    deps.assertionStore.list(missionId),
    deps.checkpointStore.list(missionId),
    buildMissionControlEnvironmentSummary(deps.config, deps.git, deps.cwd),
    deps.config.loadLayers(deps.cwd),
    deps.git.getState(deps.cwd),
      buildMissionControlMemorySnapshot({
        correctionStore: deps.correctionStore,
        learningStore: deps.learningStore,
        ratchetStore: deps.ratchetStore,
        projectGraphStore: deps.projectGraphStore,
        cwd: deps.cwd,
      }),
      loadPendingHandoffs(deps.handoffStore),
      taskBoardPromise,
  ]);

  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

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

  // Feature rows
  const featureRows: MissionControlFeatureRow[] = features.map((f) => {
    const preview = taskPreviewById.get(f.id);

    return {
      id: f.id,
      title: f.title,
      status: f.status,
      milestoneId: f.milestoneId,
      workerType: f.workerType,
      hasReport: f.report !== undefined && f.report !== null,
      blockedByIds: preview?.blockedBy?.map((item) => item.id) ?? [],
      blockedByLabel: buildBlockedByLabel(preview?.blockedBy ?? []),
    };
  });

  // Active feature: first assigned or in-progress
  const activeFeature = findActiveFeature(taskPreviews);

  // Progress log: mission/feature/assertion/checkpoint events only.
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
    kind: mp.milestone.kind ?? "work",
    profile: mp.milestone.profile ?? "custom",
  }));

  // Feature progress
  const doneCount = features.filter((f) => f.status === "done").length;
  const activeCount = features.filter(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  ).length;
  const blockedCount = features.filter((f) => f.status === "blocked").length;
  const queuedCount = features.filter((f) => f.status === "pending").length;
  const workerTypes = [...new Set(features.map((feature) => feature.workerType))];
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

  const replyInbox = replies ? buildReplyInbox(features, replies) : undefined;

  // Principle effectiveness rollup (piggybacks on includeReplies because
  // the reply ingest is what produces most of the decided outcomes).
  // Reuse the in-memory outcomes cache from ingest to avoid re-reading
  // outcomes.jsonl.
  const principleEffectiveness = options.includeReplies === true
    ? await loadPrincipleEffectiveness(deps, ingest.outcomesCache)
    : undefined;

  // Conductor screen data
  const agentGrid = buildAgentGrid(features, pendingHandoffs);
  const missionMilestones = mission.milestones;
  const dispatchQueue = buildDispatchQueue(features, missionMilestones);

  const eventStream = buildEventStream(progressLog, pendingHandoffs, replies ?? []);
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
    tokenCounters: null, // No telemetry infrastructure yet
    missionOverview,
    activeFeature,
    features: featureRows,
    taskPreviews,
    session: sessionSidebar,
    pendingHandoffs,
    configSummary: {
      configSource: env.status.configSource,
      cassAvailable: env.status.cassAvailable,
      gitAvailable: env.status.gitAvailable,
      checks,
      missionDirectory: `.maestro/missions/${mission.id}`,
      workerTypes,
      backgroundMode,
    },
    configInspector: buildConfigInspector(configLayers, checks, features, []),
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
    home: null,
  };
}

export async function buildHomeSnapshot(
  deps: HomeSnapshotDeps,
  options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const taskBoardPromise = options.includeTaskBoard === true
    ? buildTaskBoard(deps.taskStore)
    : Promise.resolve(undefined);
  const [env, configLayers, gitState, memorySnapshot, pendingHandoffs, taskBoard] = await Promise.all([
    buildMissionControlEnvironmentSummary(deps.config, deps.git, deps.cwd),
    deps.config.loadLayers(deps.cwd),
    deps.git.isRepo(deps.cwd).then((isRepo) => isRepo ? deps.git.getState(deps.cwd) : Promise.resolve(undefined)),
    buildMissionControlMemorySnapshot({
      correctionStore: deps.correctionStore,
      learningStore: deps.learningStore,
      ratchetStore: deps.ratchetStore,
      projectGraphStore: deps.projectGraphStore,
      cwd: deps.cwd,
    }),
    loadPendingHandoffs(deps.handoffStore),
    taskBoardPromise,
  ]);
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
      : "Open a git repository to track missions, checkpoints, and handoffs here.";

  const actions = buildHomeActions(status, checks);

  const agentGrid = buildAgentGrid([], pendingHandoffs);
  // Replies in home mode: list without ingest (home mode has no mission to
  // update). Home surface is purely read-only per Mission Control contracts.
  const homeReplies = options.includeReplies === true && deps.replyStore
    ? await safeListReplies(deps.replyStore)
    : undefined;
  const homeReplyInbox = homeReplies ? buildReplyInbox([], homeReplies) : undefined;
  const homeEventStream = buildEventStream([], pendingHandoffs, homeReplies ?? []);

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
    session: gitState
      ? {
        branch: gitState.branch,
        workingTreeClean: gitState.workingTreeClean,
        diffStat: gitState.diffStat,
        changedFiles: gitState.changedFiles,
        fileChanges: gitState.fileChanges ?? [],
      }
      : null,
    pendingHandoffs,
    configSummary: {
      configSource: status.configSource,
      cassAvailable: status.cassAvailable,
      gitAvailable: status.gitAvailable,
      checks,
      missionDirectory: null,
      workerTypes: [],
      backgroundMode,
    },
    configInspector: buildConfigInspector(configLayers, checks, [], []),
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
    principleEffectiveness: options.includeReplies === true
      ? await loadPrincipleEffectiveness(deps)
      : undefined,
    home: {
      headline,
      summary,
        locationLabel: status.gitAvailable ? deps.cwd : "Outside a git repository",
      checks,
      actions,
      pendingHandoffs,
    },
  };
}

async function loadPendingHandoffs(
  handoffStore: HandoffStorePort | undefined,
): Promise<readonly MissionControlHomeHandoff[]> {
  if (!handoffStore) return [];
  try {
    const pending = await handoffStore.list({ status: "pending" });
    return pending.map(mapUkiHandoffToHomeHandoff);
  } catch {
    // Store errors should not break the snapshot projection. A missing
    // .maestro/handoffs directory simply means no pending work.
    return [];
  }
}

export function mapUkiHandoffToHomeHandoff(
  handoff: UkiHandoff,
): MissionControlHomeHandoff {
  const content = handoff.content;
  const sitrepParts = [content.sessionCore];
  if (content.decisions.length > 0) {
    sitrepParts.push(content.decisions[0]!);
  }
  return {
    id: handoff.id,
    message: content.summary,
    agent: handoff.agent,
    timestamp: handoff.timestamp,
    sessionId: handoff.sessionId,
    sitrep: sitrepParts.join(" -- "),
    quickstart: content.nextAction,
  };
}

function findActiveFeature(taskPreviews: readonly TaskPreviewPane[]): MissionControlFeatureDetail | null {
  return taskPreviews.find(
    (feature) => feature.status === "assigned" || feature.status === "in-progress" || feature.status === "review",
  ) ?? taskPreviews.find((feature) => feature.status === "pending") ?? null;
}

const MEMORY_SNAPSHOT_TTL_MS = 30_000;
const memorySnapshotCache = new Map<string, CacheEntry<MissionControlMemorySnapshot | null>>();

async function buildMissionControlMemorySnapshot(
  deps: {
    correctionStore?: CorrectionStorePort;
    learningStore?: LearningStorePort;
    ratchetStore?: RatchetStorePort;
    projectGraphStore?: ProjectGraphStorePort;
    cwd: string;
  },
): Promise<MissionControlMemorySnapshot | null> {
  if (!deps.correctionStore || !deps.learningStore || !deps.ratchetStore) {
    return null;
  }

  const hit = cached(memorySnapshotCache.get(deps.cwd));
  if (hit !== undefined) return hit;

  const [corrections, rawLearnings, compiledLearnings, ratchetSuite, ratchetBaseline, graphContext] = await Promise.all([
    deps.correctionStore.list(),
    deps.learningStore.listRaw(),
    deps.learningStore.readCompiled(),
    deps.ratchetStore.getSuite(),
    deps.ratchetStore.getBaseline(),
    deps.projectGraphStore
      ? getGraphContext(deps.projectGraphStore, basename(deps.cwd))
      : Promise.resolve(undefined),
  ]);
  const stats = buildMemoryStats({
    corrections,
    rawLearningCount: rawLearnings.length,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphProjects: graphContext?.totalProjects ?? 0,
    graphLinks: graphContext?.totalEdges ?? 0,
  });

  const result: MissionControlMemorySnapshot = {
    stats,
    corrections,
    rawLearnings,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphContext: graphContext
      ? {
          currentProject: graphContext.currentProject,
          relationships: graphContext.relationships.map((relationship) => ({
            project: relationship.project,
            direction: relationship.direction,
            edge: relationship.edge,
          })),
          totalProjects: graphContext.totalProjects,
          totalEdges: graphContext.totalEdges,
        }
      : undefined,
  };
  setCachedEntry(memorySnapshotCache, deps.cwd, result, MEMORY_SNAPSHOT_TTL_MS);
  return result;
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
    workerType: active.workerType,
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

function buildIgnoredProjectOverrideChecks(projectConfig: import("@/infra/domain/config-types.js").MaestroConfig | undefined): DoctorCheck[] {
  return listIgnoredProjectConfigKeys(projectConfig).map((keyPath) => ({
    name: `ignored-${keyPath.replaceAll(".", "-")}`,
    status: "warn" as const,
    message: `${keyPath} is set in project config but only global config is used`,
    fix: "Remove the project value or set it in ~/.maestro/config.yaml instead",
  }));
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

// ---------------------------------------------------------------------------
// Conductor screen builders
// ---------------------------------------------------------------------------

const STALE_THRESHOLD_MS = 60 * 60 * 1000; // 1 hour

export function buildAgentGrid(
  features: readonly Feature[],
  pendingHandoffs: readonly MissionControlHomeHandoff[],
): readonly AgentGridRow[] {
  const byWorker = new Map<string, Feature[]>();
  for (const f of features) {
    const bucket = byWorker.get(f.workerType) ?? [];
    bucket.push(f);
    byWorker.set(f.workerType, bucket);
  }

  const pendingHandoffsByAgent = new Map<string, number>();
  for (const handoff of pendingHandoffs) {
    pendingHandoffsByAgent.set(
      handoff.agent,
      (pendingHandoffsByAgent.get(handoff.agent) ?? 0) + 1,
    );
  }

  const rows: AgentGridRow[] = [];
  const workerTypes = new Set<string>([
    ...byWorker.keys(),
    ...pendingHandoffsByAgent.keys(),
  ]);

  for (const workerType of workerTypes) {
    const workerFeatures = byWorker.get(workerType) ?? [];
    const active = workerFeatures.find(
      (f) => f.status === "assigned" || f.status === "in-progress",
    );
    const hasReview = workerFeatures.some((f) => f.status === "review");
    const allDone = workerFeatures.length > 0 && workerFeatures.every((f) => f.status === "done");
    const pendingHandoffCount = pendingHandoffsByAgent.get(workerType) ?? 0;
    const isStale = active !== undefined
      && (Date.now() - new Date(active.updatedAt).getTime()) > STALE_THRESHOLD_MS;

    let status: InferredAgentStatus;
    if (isStale) status = "stale";
    else if (active) status = "active";
    else if (hasReview || pendingHandoffCount > 0) status = "waiting";
    else if (allDone) status = "completed";
    else status = "waiting";

    rows.push({
      workerType,
      status,
      activeFeatureId: active?.id,
      activeFeatureTitle: active?.title,
      lastActivityAt: active?.updatedAt,
      featureCount: workerFeatures.length,
      completedCount: workerFeatures.filter((f) => f.status === "done").length,
      pendingHandoffCount,
    });
  }

  // Sort: active first, then waiting, then stale, then completed
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
      const fit = recommendWorkerFit(f.workerType, features);
      return {
        featureId: f.id,
        featureTitle: f.title,
        milestoneId: f.milestoneId,
        milestoneTitle: milestone?.title ?? f.milestoneId,
        milestoneOrder: milestone?.order ?? 0,
        workerType: f.workerType,
        fitReason: fit.reason,
      };
    })
    .sort((a, b) => a.milestoneOrder - b.milestoneOrder);
}

export function buildEventStream(
  progressLog: readonly MissionControlEvent[],
  pendingHandoffs: readonly MissionControlHomeHandoff[],
  replies: readonly WorkerReply[] = [],
): readonly EventStreamEntry[] {
  const entries: EventStreamEntry[] = [];
  const baseMs = getEventStreamBaseMs(progressLog);

  // Reuse existing progress log events
  for (const event of progressLog) {
    entries.push({
      timestamp: event.timestamp,
      relativeMs: event.relativeMs,
      kind: event.kind,
      title: event.title,
      detail: event.detail,
    });
  }

  for (const h of pendingHandoffs) {
    const handoffMs = new Date(h.timestamp).getTime();
    entries.push({
      timestamp: h.timestamp,
      relativeMs: baseMs === undefined || Number.isNaN(handoffMs)
        ? 0
        : handoffMs - baseMs,
      kind: "handoff",
      title: `Pending handoff from ${h.agent}`,
      detail: h.message,
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

  // Sort descending by timestamp, cap at 200
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

interface IngestResult {
  readonly replies: readonly WorkerReply[];
  /** Cached outcomes (plus any appends from this ingest pass) for downstream aggregators. */
  readonly outcomesCache?: readonly PrincipleOutcomeRecord[];
}

async function loadAndIngestReplies(
  deps: SnapshotDeps,
  missionId: string,
): Promise<IngestResult> {
  if (!deps.replyStore) return { replies: [] };
  try {
    const replies = (await deps.replyStore.list()).filter((reply) => reply.missionId === missionId);
    if (replies.length === 0) return { replies: [] };

    // Cache outcomes once so the recorder doesn't re-read the JSONL per
    // handoff per reply (N*M disk reads -> 1). Appends are tracked
    // in-memory so subsequent recorder calls and the effectiveness
    // aggregator see the post-ingest state.
    const outcomesCache: PrincipleOutcomeRecord[] | undefined = deps.principleStore
      ? [...(await deps.principleStore.listOutcomes())]
      : undefined;

    const recordPrincipleOutcomes = buildPrincipleRecorder(deps, missionId, outcomesCache);

    for (const reply of replies) {
      // Fast path: skip already-ingested replies without entering the
      // usecase at all. The usecase also defends against this, but this
      // avoids the extra function call and conditional wiring.
        if (await deps.replyStore.isIngested(missionId, reply.featureId)) continue;
      try {
        await ingestReply(
          {
            missionStore: deps.missionStore,
            featureStore: deps.featureStore,
            assertionStore: deps.assertionStore,
            replyStore: deps.replyStore,
            baseDir: deps.cwd,
            ...(recordPrincipleOutcomes ? { recordPrincipleOutcomes } : {}),
          },
          missionId,
          reply.featureId,
        );
      } catch {
        // Snapshot projection must not throw on a single bad reply.
        // The reply remains on disk; next poll will retry.
      }
    }

    return { replies, outcomesCache };
  } catch {
    return { replies: [] };
  }
}

/**
 * Build a principle-outcome recorder that marks every `pending` principle
 * row for the handoffs linked to the feature as either `helpful` or
 * `unhelpful`. The `outcomesCache` is filtered in-memory to avoid
 * re-reading outcomes.jsonl once per handoff; appends are pushed back
 * into the cache so the effectiveness aggregator sees fresh state.
 */
function buildPrincipleRecorder(
  deps: SnapshotDeps,
  missionId: string,
  outcomesCache: PrincipleOutcomeRecord[] | undefined,
): ((featureId: string, outcome: ReplyOutcome) => Promise<{ recorded: number; complete: boolean }>) | undefined {
  const principleStore = deps.principleStore;
  const handoffStore = deps.handoffStore;
  if (!principleStore || !handoffStore || !outcomesCache) return undefined;

    return async (featureId, outcome) => {
      const resolved = outcome === "completed" ? "helpful" : "unhelpful";
      try {
      const recentHandoffs = handoffStore.listRecentByFeatureRefs
        ? await handoffStore.listRecentByFeatureRefs(missionId, featureId, 25)
        : (await handoffStore.list()).filter((h) => h.content.maestroRefs.featureId === featureId);
        if (recentHandoffs.length === 0) {
          return { recorded: 0, complete: true };
        }

        let recorded = 0;
        let complete = true;
        const recordedAt = new Date().toISOString();
        for (const handoff of recentHandoffs) {
          const pending = filterPendingForHandoff(outcomesCache, handoff.id);
          for (const row of pending) {
            const record: PrincipleOutcomeRecord = {
            principleId: row.principleId,
            handoffId: handoff.id,
            featureId,
            missionId,
              outcome: resolved,
              recordedAt,
            };
            if (await principleStore.recordOutcome(record)) {
              outcomesCache.push(record);
              recorded += 1;
              continue;
            }
            complete = false;
          }
        }
        return { recorded, complete };
      } catch {
        return { recorded: 0, complete: false };
      }
    };
  }

async function safeListReplies(
  replyStore: ReplyStorePort,
): Promise<readonly WorkerReply[]> {
  try {
    return await replyStore.list();
  } catch {
    return [];
  }
}

/**
 * Return the effective `pending` outcomes for a single handoff, using the
 * latest record per (principleId, handoffId) pair. Pure in-memory filter
 * over the pre-fetched cache; mirrors JsonlPrincipleStoreAdapter.listPendingOutcomesForHandoff.
 */
function filterPendingForHandoff(
  outcomes: readonly PrincipleOutcomeRecord[],
  handoffId: string,
): readonly PrincipleOutcomeRecord[] {
  const latestByPrinciple = new Map<string, PrincipleOutcomeRecord>();
  for (const record of outcomes) {
    if (record.handoffId !== handoffId) continue;
    const existing = latestByPrinciple.get(record.principleId);
    if (!existing || existing.recordedAt <= record.recordedAt) {
      latestByPrinciple.set(record.principleId, record);
    }
  }
  return [...latestByPrinciple.values()].filter((r) => r.outcome === "pending");
}

async function loadPrincipleEffectiveness(
  deps: SnapshotDeps | HomeSnapshotDeps,
  cachedOutcomes?: readonly PrincipleOutcomeRecord[],
): Promise<readonly PrincipleEffectivenessRow[] | undefined> {
  const principleStore = deps.principleStore;
  const handoffStore = deps.handoffStore;
  if (!principleStore) return undefined;
  try {
    const [principles, outcomes, handoffs] = await Promise.all([
      principleStore.list(),
      cachedOutcomes !== undefined
        ? Promise.resolve(cachedOutcomes)
        : principleStore.listOutcomes(),
      handoffStore ? handoffStore.list() : Promise.resolve<readonly UkiHandoff[]>([]),
    ]);
    return buildPrincipleEffectivenessRows(principles, outcomes, handoffs);
  } catch {
    return undefined;
  }
}

export function buildPrincipleEffectivenessRows(
  principles: readonly Principle[],
  outcomes: readonly PrincipleOutcomeRecord[],
  handoffs: readonly UkiHandoff[],
): readonly PrincipleEffectivenessRow[] {
  const rollup = buildPrincipleEffectiveness(principles, outcomes);
  const principleById = new Map(principles.map((p) => [p.id, p]));
  const handoffById = new Map(handoffs.map((h) => [h.id, h]));

  // Index most recent unhelpful outcomes per principle, newest first.
  const unhelpfulByPrinciple = new Map<string, PrincipleOutcomeRecord[]>();
  for (const record of [...outcomes].sort((a, b) => b.recordedAt.localeCompare(a.recordedAt))) {
    if (record.outcome !== "unhelpful") continue;
    const bucket = unhelpfulByPrinciple.get(record.principleId) ?? [];
    if (bucket.length < 3) bucket.push(record);
    unhelpfulByPrinciple.set(record.principleId, bucket);
  }

  const rows: PrincipleEffectivenessRow[] = [];
  for (const stats of rollup.values()) {
    const principle = principleById.get(stats.principleId);
    const decided = stats.helpful + stats.unhelpful;
    const examples = (unhelpfulByPrinciple.get(stats.principleId) ?? [])
      .map((r) => {
        const handoff = handoffById.get(r.handoffId);
        const title = handoff?.content.summary ?? r.handoffId;
        return `${r.handoffId}: ${title}`;
      });

    rows.push({
      id: stats.principleId,
      name: principle?.name ?? stats.principleId,
      mode: principle?.mode ?? "advisory",
      helpful: stats.helpful,
      unhelpful: stats.unhelpful,
      pending: stats.pending,
      total: stats.total,
      effectivenessPct: stats.effectiveness === undefined
        ? undefined
        : Math.round(stats.effectiveness * 100),
      lowSample: decided < PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
      recentKickbackExamples: examples,
    });
  }

  // Worst first (lowest effectiveness). Principles with no decided outcomes
  // sort last so the scoreboard leads with actionable signal.
  rows.sort((a, b) => {
    const ae = a.effectivenessPct ?? 101;
    const be = b.effectivenessPct ?? 101;
    if (ae !== be) return ae - be;
    const aDecided = a.helpful + a.unhelpful;
    const bDecided = b.helpful + b.unhelpful;
    return bDecided - aDecided;
  });
  return rows;
}

export function buildReplyInbox(
  features: readonly Feature[],
  replies: readonly WorkerReply[],
): readonly ReplyInboxEntry[] {
  const featureById = new Map(features.map((f) => [f.id, f]));
  const entries: ReplyInboxEntry[] = replies.map((reply) => {
    const feature = featureById.get(reply.featureId);
    return {
      featureId: reply.featureId,
      outcome: reply.outcome,
      writtenAt: reply.writtenAt,
      writtenBy: reply.writtenBy,
      featureTitle: feature?.title,
      featureStatus: feature?.status,
      pending: isReplyPending(reply, feature),
      notes: reply.notes,
    };
  });
  entries.sort((a, b) => b.writtenAt.localeCompare(a.writtenAt));
  return entries;
}

function isReplyPending(reply: WorkerReply, feature: Feature | undefined): boolean {
  if (!feature) return true;
  if (reply.outcome === "completed") return feature.status !== "done";
  if (reply.outcome === "abandoned") return feature.status !== "blocked";
  // kicked-back: the loop lands at pending
  return feature.status !== "pending";
}

export async function buildTaskBoard(
  taskStore?: TaskQueryPort,
): Promise<TaskBoardSnapshot | null> {
  if (!taskStore) return null;
  const tasks = await taskStore.all();
  if (tasks.length === 0) return null;

  const columns = Object.fromEntries(
    TASK_STATUSES.map((s) => [s, [] as TaskBoardItem[]]),
  ) as Record<TaskStatus, TaskBoardItem[]>;

  for (const task of tasks) {
    const item: TaskBoardItem = {
      id: task.id,
      title: task.title,
      status: task.status,
      priority: task.priority,
      assignee: task.assignee,
      labels: task.labels,
      blockedByCount: task.blockedBy.length,
    };
    columns[task.status]?.push(item);
  }

  // Sort each column by priority (lower = higher priority), then createdAt
  for (const status of TASK_STATUSES) {
    columns[status]!.sort((a, b) => a.priority - b.priority);
  }

  return { columns, totalCount: tasks.length };
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

async function buildMissionControlEnvironmentSummary(
  config: ConfigPort,
  git: GitPort,
  cwd: string,
): Promise<{ status: StatusReport; checks: readonly DoctorCheck[] }> {
  const [
    projectConfigExists,
    globalConfigExists,
    gitAvailable,
  ] = await Promise.all([
    config.exists("project", cwd),
    config.exists("global", cwd),
    git.isRepo(cwd),
  ]);

  const configSource: StatusReport["configSource"] = projectConfigExists
    ? "project"
    : globalConfigExists
      ? "global"
      : "none";

  return {
    status: {
      initialized: projectConfigExists || globalConfigExists,
      configSource,
      pendingHandoffs: [],
      cassAvailable: false,
      gitAvailable,
    },
    checks: [
      {
        name: "git",
        status: gitAvailable ? "ok" : "fail",
        message: gitAvailable ? "Git repository detected" : "Not inside a git repository",
        fix: gitAvailable ? undefined : "Run: git init",
      },
      {
        name: "project-config",
        status: projectConfigExists ? "ok" : "warn",
        message: projectConfigExists ? "Project config found at .maestro/config.yaml" : "No project config found",
        fix: projectConfigExists ? undefined : "Run: maestro init",
      },
      {
        name: "global-config",
        status: globalConfigExists ? "ok" : "warn",
        message: globalConfigExists ? "Global config found at ~/.maestro/config.yaml" : "No global config found",
        fix: globalConfigExists ? undefined : "Run: maestro init --global",
      },
    ],
  };
}
