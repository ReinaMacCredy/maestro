/**
 * Build a MissionControlSnapshot from existing stores.
 * Polls once -- no subscriptions, no event tailing.
 */
import { basename } from "node:path";
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
import {
  type Mission,
  type Feature,
  generateMissionReport,
  type MissionReport,
  getValidFeatureTransitions,
} from "@/features/mission";
import { getMissionControlBackgroundMode, listIgnoredProjectConfigKeys } from "@/shared/domain/ui-config.js";
import type { DoctorCheck, StatusReport } from "@/infra/domain/status-types.js";
import { getGraphContext } from "@/features/graph";
import { deriveEvents } from "./events.js";
import { buildConfigInspector } from "./config-inspector.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlMilestoneRow,
  MissionControlHomeAction,
  MissionControlHomeHandoff,
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
}

export interface SnapshotBuildOptions {
  readonly probeWorkers?: boolean;
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
  _options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const [
    report,
    features,
    assertions,
    checkpoints,
    env,
    configLayers,
    gitState,
    memorySnapshot,
    pendingHandoffs,
  ] = await Promise.all([
    generateMissionReport(
      deps.missionStore,
      deps.featureStore,
      deps.assertionStore,
      missionId,
    ),
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
    ]);

  const mission = report.mission;
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
  // Phase 3 strip: worker progress events no longer exist.
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
    home: null,
  };
}

export async function buildHomeSnapshot(
  deps: HomeSnapshotDeps,
  cwd: string,
  _options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const [env, configLayers, gitState, memorySnapshot, pendingHandoffs] = await Promise.all([
    buildMissionControlEnvironmentSummary(deps.config, deps.git, cwd),
    deps.config.loadLayers(cwd),
    deps.git.isRepo(cwd).then((isRepo) => isRepo ? deps.git.getState(cwd) : Promise.resolve(undefined)),
    buildMissionControlMemorySnapshot({
      correctionStore: deps.correctionStore,
      learningStore: deps.learningStore,
      ratchetStore: deps.ratchetStore,
      projectGraphStore: deps.projectGraphStore,
      cwd,
    }),
    loadPendingHandoffs(deps.handoffStore),
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
    home: {
      headline,
      summary,
      locationLabel: status.gitAvailable ? cwd : "Outside a git repository",
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
let memorySnapshotCache: { value: MissionControlMemorySnapshot | null; expiresAt: number } | undefined;

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

  if (memorySnapshotCache && memorySnapshotCache.expiresAt > Date.now()) {
    return memorySnapshotCache.value;
  }

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
  memorySnapshotCache = { value: result, expiresAt: Date.now() + MEMORY_SNAPSHOT_TTL_MS };
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

function buildIgnoredProjectOverrideChecks(projectConfig: import("../../domain/types.js").MaestroConfig | undefined): DoctorCheck[] {
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
    // Phase 3 strip: agentSummary was derived from runtime state. Without
    // a runtime store there is no per-agent activity to report.
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
      // Phase 1 strip: handoff store + CASS are gone; the fields
      // survive on StatusReport only to keep the TUI types stable
      // until Phase 2/3 remove them.
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
