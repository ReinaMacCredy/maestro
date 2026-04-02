/**
 * Build a MissionControlSnapshot from existing stores.
 * Polls once -- no subscriptions, no event tailing.
 */
import type { MissionStorePort } from "../../ports/mission-store.port.js";
import type { FeatureStorePort } from "../../ports/feature-store.port.js";
import type { AssertionStorePort } from "../../ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../../ports/checkpoint-store.port.js";
import type { HandoffStorePort } from "../../ports/handoff-store.port.js";
import type { ConfigPort } from "../../ports/config.port.js";
import type { CassPort } from "../../ports/cass.port.js";
import type { GitPort } from "../../ports/git.port.js";
import type { RuntimeStorePort } from "../../ports/runtime-store.port.js";
import type { RuntimeEventStorePort } from "../../ports/runtime-event-store.port.js";
import type { Mission, Feature } from "../../domain/mission-types.js";
import type { RuntimeState, WorkerRuntime } from "../../domain/runtime-types.js";
import type { DoctorCheck, StatusReport } from "../../domain/types.js";
import type { RuntimeEventRecord } from "../../domain/worker-types.js";
import { CASS_INSTALL_HINT } from "../../domain/defaults.js";
import { generateMissionReport, type MissionReport } from "../../usecases/mission-report.usecase.js";
import { checkStatus } from "../../usecases/check-status.usecase.js";
import { runDoctor } from "../../usecases/run-doctor.usecase.js";
import { getValidFeatureTransitions } from "../../domain/mission-state.js";
import { classifyRuntime } from "../../usecases/runtime-supervision.usecase.js";
import { getWorkerHealthRows } from "../../usecases/worker-health.usecase.js";
import { deriveEvents } from "./events.js";
import { buildConfigInspector } from "./config-inspector.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlWorkerPane,
  MissionControlMilestoneRow,
  MissionControlHomeAction,
  MissionControlHomeHandoff,
  MissionControlRuntimeProcessRow,
  BlockedByRef,
  TaskPreviewPane,
  MissionOverviewPane,
  DependencyMapRow,
} from "./types.js";

export interface SnapshotDeps {
  missionStore: MissionStorePort;
  featureStore: FeatureStorePort;
  assertionStore: AssertionStorePort;
  checkpointStore: CheckpointStorePort;
  handoffStore: HandoffStorePort;
  config: ConfigPort;
  cass: CassPort;
  git: GitPort;
  runtimeStore: RuntimeStorePort;
  runtimeEventStore: RuntimeEventStorePort;
  cwd: string;
}

export interface HomeSnapshotDeps {
  handoffStore: HandoffStorePort;
  config: ConfigPort;
  cass: CassPort;
  git: GitPort;
}

interface RuntimeView {
  readonly runtimeState: RuntimeState;
  readonly lastSeenAgeMs: number;
  readonly failureReason?: string;
  readonly retryCount: number;
  readonly agent: string;
  readonly sessionId?: string;
  readonly startedAtMs: number;
  readonly leaseRemainingMs: number;
}

interface RuntimeTelemetry {
  readonly currentActivity?: string;
  readonly lastOutputAgeMs?: number;
  readonly outputLines: readonly {
    timestamp: string;
    kind: "status" | "stdout" | "stderr";
    text: string;
  }[];
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
  ): Promise<MissionControlSnapshot> {
  const [
    report,
    features,
    assertions,
    checkpoints,
    env,
    configLayers,
    gitState,
    runtimes,
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
    buildMissionControlEnvironmentSummary(deps.handoffStore, deps.config, deps.cass, deps.git, deps.cwd),
    deps.config.loadLayers(deps.cwd),
    deps.git.getState(deps.cwd),
    deps.runtimeStore.list(missionId),
  ]);

    const mission = report.mission;
    const now = Date.now();
    const startMs = new Date(mission.approvedAt ?? mission.createdAt).getTime();
    const runtimeEventsByFeature = new Map(
      await Promise.all(
        features.map(async (feature) => [
          feature.id,
          await deps.runtimeEventStore.listByFeature(missionId, feature.id),
        ] as const),
      ),
    );
    const runtimeByFeature = new Map(
      runtimes.map((runtime) => [runtime.featureId, buildRuntimeView(runtime, now)]),
    );
    const runtimeTelemetryByFeature = new Map(
      features.map((feature) => [feature.id, buildRuntimeTelemetry(runtimeEventsByFeature.get(feature.id) ?? [], now)]),
    );
    const featureGraph = buildFeatureGraph(features);
  const taskPreviews = features.map((feature) =>
    buildTaskPreview(feature, report, runtimeByFeature, featureGraph.get(feature.id))
  );
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

  // Active worker
    const activeWorker = buildActiveWorker(features, runtimeByFeature, runtimeTelemetryByFeature, startMs, now);

  // Progress log
  const progressLog = deriveEvents({
    mission,
    features,
      assertions,
      checkpoints,
      milestoneProgress: report.milestones,
      workerEvents: [...runtimeEventsByFeature.values()].flat(),
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
  const pendingHandoffs = env.status.pendingHandoffs.map(mapPendingHandoff);
  const workerTypes = [...new Set(features.map((feature) => feature.workerType))];
  const activeMilestone = milestones.find((m) => m.status === "executing" || m.status === "validating");
  const gateLabel = activeMilestone?.kind === "gate" ? activeMilestone.title : null;
  const gateBlocked = Boolean(activeMilestone && activeMilestone.kind === "gate"
    && features.some((f) => f.milestoneId === activeMilestone.id && f.status === "blocked"));
  const missionOverview = buildMissionOverview(
    mission,
    features,
    featureGraph,
    runtimeByFeature,
    {
      doneCount,
      blockedCount,
      activeCount,
      currentMilestoneId: activeMilestone?.id ?? null,
      currentMilestone: activeMilestone?.title ?? null,
      gateLabel,
    },
  );
  const sessionSidebar = buildSessionSidebar(gitState, activeWorker);

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
      activeWorker,
      session: sessionSidebar,
      pendingHandoffs,
        configSummary: {
        configSource: env.status.configSource,
        cassAvailable: env.status.cassAvailable,
        gitAvailable: env.status.gitAvailable,
        checks: env.checks,
        missionDirectory: `.maestro/missions/${mission.id}`,
        workerTypes,
        },
        configInspector: buildConfigInspector(configLayers, env.checks, features, env.status.configSource),
        workerHealth: await getWorkerHealthRows(configLayers.effective.workers ?? {}, {
          activeWorkers: features
            .filter((feature) => feature.status === "assigned" || feature.status === "in-progress" || feature.status === "review")
            .map((feature) => runtimeByFeature.get(feature.id)?.agent ?? "unknown")
            .filter((agent) => agent !== "unknown"),
        }),
        runtimeProcesses: buildRuntimeProcesses(mission, features, runtimeByFeature, runtimeTelemetryByFeature, activeWorker),
        progressLog,
      milestones,
      gateBlocked,
      gateLabel,
      canPause: mission.status === "executing",
      canResume: mission.status === "paused",
      home: null,
  };
}

export async function buildHomeSnapshot(
  deps: HomeSnapshotDeps,
  cwd: string,
): Promise<MissionControlSnapshot> {
  const [env, configLayers, gitState] = await Promise.all([
    buildMissionControlEnvironmentSummary(deps.handoffStore, deps.config, deps.cass, deps.git, cwd),
    deps.config.loadLayers(cwd),
    deps.git.isRepo(cwd).then((isRepo) => isRepo ? deps.git.getState(cwd) : Promise.resolve(undefined)),
  ]);
  const { status, checks } = env;

  const headline = status.gitAvailable
    ? "No missions yet"
    : "No project detected";

  const summary = status.gitAvailable
    ? "Initialize this repository, then create your first mission."
    : status.initialized
      ? "Global setup is ready. Open a project repository to start tracking missions here."
      : "Open a git repository to track missions, checkpoints, and handoffs here.";

  const actions = buildHomeActions(status, checks);
  const pendingHandoffs = status.pendingHandoffs.map(mapPendingHandoff);

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
      activeWorker: null,
      session: gitState
        ? {
          agent: undefined,
          sessionId: undefined,
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
    },
      configInspector: buildConfigInspector(configLayers, checks, [], status.configSource),
      workerHealth: await getWorkerHealthRows(configLayers.effective.workers ?? {}),
      runtimeProcesses: [],
    progressLog: [],
    milestones: [],
    gateBlocked: false,
    gateLabel: null,
    canPause: false,
    canResume: false,
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

function findActiveFeature(taskPreviews: readonly TaskPreviewPane[]): MissionControlFeatureDetail | null {
  return taskPreviews.find(
    (feature) => feature.status === "assigned" || feature.status === "in-progress" || feature.status === "review",
  ) ?? taskPreviews.find((feature) => feature.status === "pending") ?? null;
}

function buildActiveWorker(
  features: readonly Feature[],
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  telemetryByFeature: ReadonlyMap<string, RuntimeTelemetry>,
  startMs: number,
  nowMs: number,
): MissionControlWorkerPane | null {
  const active = features.find(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  );

  if (!active) return null;

  const runtime = runtimeByFeature.get(active.id);
  const telemetry = telemetryByFeature.get(active.id);
  const featureStartMs = runtime?.startedAtMs ?? new Date(active.updatedAt).getTime();

  return {
    featureId: active.id,
    featureTitle: active.title,
    workerType: active.workerType,
    status: active.status,
    elapsedMs: nowMs - featureStartMs,
    report: active.report ?? null,
    runtimeState: presentRuntimeState(runtime, active.status),
    lastSeenAgeMs: runtime?.lastSeenAgeMs,
    failureReason: runtime?.failureReason,
      retryCount: runtime?.retryCount,
      agent: runtime?.agent,
      sessionId: runtime?.sessionId,
      currentActivity: telemetry?.currentActivity,
      lastOutputAgeMs: telemetry?.lastOutputAgeMs,
    };
  }

function buildHomeActions(
  status: Awaited<ReturnType<typeof checkStatus>>,
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
    detail: "Verify git, CASS, and config health before starting work.",
  });

  return actions;
}

function buildRuntimeProcesses(
  mission: Mission,
  features: readonly Feature[],
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  runtimeTelemetryByFeature: ReadonlyMap<string, RuntimeTelemetry>,
  activeWorker: MissionControlWorkerPane | null,
): readonly MissionControlRuntimeProcessRow[] {
  const milestoneById = new Map(mission.milestones.map((milestone) => [milestone.id, milestone]));
  return features
      .filter((feature) => {
        const runtime = runtimeByFeature.get(feature.id);
        if (runtime) {
          return runtime.runtimeState !== "completed"
            && !(feature.status === "pending" && runtime.runtimeState === "starting");
        }
        return feature.status === "assigned"
          || feature.status === "in-progress"
        || feature.status === "review";
        })
          .map((feature) => {
            const runtime = runtimeByFeature.get(feature.id);
            const telemetry = runtimeTelemetryByFeature.get(feature.id);
            const milestone = milestoneById.get(feature.milestoneId);
            return {
            featureId: feature.id,
            title: feature.title,
            milestoneTitle: milestone?.title,
            profile: milestone?.profile,
            status: feature.status,
            workerType: feature.workerType,
        hasReport: feature.report !== undefined && feature.report !== null,
        isLive: runtime
          ? presentRuntimeState(runtime, feature.status) === "live"
          : activeWorker?.featureId === feature.id,
        runtimeState: presentRuntimeState(runtime, feature.status),
        lastSeenAgeMs: runtime?.lastSeenAgeMs,
          failureReason: runtime?.failureReason,
          retryCount: runtime?.retryCount,
          agent: runtime?.agent,
          sessionId: runtime?.sessionId,
          currentActivity: telemetry?.currentActivity,
          lastOutputAgeMs: telemetry?.lastOutputAgeMs,
          leaseRemainingMs: runtime?.leaseRemainingMs,
          outputLines: telemetry?.outputLines,
        };
      });
    }

function buildTaskPreview(
  active: Feature,
  report: MissionReport,
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  graphEntry?: FeatureGraphEntry,
): TaskPreviewPane {
  const milestone = report.mission.milestones.find((m) => m.id === active.milestoneId);
  const runtime = runtimeByFeature.get(active.id);

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
    runtimeState: presentRuntimeState(runtime, active.status),
    lastSeenAgeMs: runtime?.lastSeenAgeMs,
    failureReason: runtime?.failureReason,
    retryCount: runtime?.retryCount,
    agent: runtime?.agent,
    sessionId: runtime?.sessionId,
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
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  summary: {
    doneCount: number;
    blockedCount: number;
    activeCount: number;
    currentMilestoneId: string | null;
    currentMilestone: string | null;
    gateLabel: string | null;
  },
): MissionOverviewPane {
  const agentCounts = new Map<string, number>();
  for (const feature of features) {
    const runtime = runtimeByFeature.get(feature.id);
    if (!runtime?.agent || runtime.agent === "unknown") continue;
    if (!(feature.status === "assigned" || feature.status === "in-progress" || feature.status === "review")) {
      continue;
    }
    agentCounts.set(runtime.agent, (agentCounts.get(runtime.agent) ?? 0) + 1);
  }

  return {
    missionLabel: `Mission: ${mission.title}`,
    statusLabel: mission.status,
    activeCount: summary.activeCount,
    doneCount: summary.doneCount,
    totalCount: features.length,
    blockedCount: summary.blockedCount,
    currentMilestone: summary.currentMilestone,
    gateLabel: summary.gateLabel,
    agentSummary: [...agentCounts.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .map(([agent, count]) => ({ agent, count })),
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
  activeWorker: MissionControlWorkerPane | null,
) {
  return {
    agent: activeWorker?.agent,
    sessionId: activeWorker?.sessionId,
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

function buildRuntimeView(runtime: WorkerRuntime, nowMs: number): RuntimeView {
  const classification = classifyRuntime(runtime, nowMs);
  return {
    runtimeState: classification.runtimeState,
    lastSeenAgeMs: classification.lastSeenAgeMs,
    failureReason: runtime.failureReason,
    retryCount: runtime.recoveryMetadata.retryCount,
      agent: runtime.agent,
      sessionId: runtime.sessionId,
      startedAtMs: classification.startedAtMs,
      leaseRemainingMs: Math.max(0, new Date(runtime.leaseExpiresAt).getTime() - nowMs),
    };
  }

function buildRuntimeTelemetry(
  events: readonly RuntimeEventRecord[],
  nowMs: number,
): RuntimeTelemetry {
  const outputLines = events
    .filter((event) =>
      event.kind !== "heartbeat"
      && typeof event.text === "string"
      && event.text.trim().length > 0,
    )
    .slice(-8)
    .map((event) => ({
      timestamp: event.timestamp,
      kind: event.kind as "status" | "stdout" | "stderr",
      text: event.text!.trim(),
    }));
  const lastOutput = [...events]
    .reverse()
    .find((event) =>
      (event.kind === "stdout" || event.kind === "stderr")
      && typeof event.text === "string"
      && event.text.trim().length > 0,
    );
  const currentActivity = outputLines.at(-1)?.text;

  return {
    currentActivity,
    lastOutputAgeMs: lastOutput ? Math.max(0, nowMs - new Date(lastOutput.timestamp).getTime()) : undefined,
    outputLines,
  };
}

function presentRuntimeState(
  runtime: RuntimeView | undefined,
  featureStatus: Feature["status"],
): RuntimeState | undefined {
  if (!runtime) return undefined;
  const isActiveStatus =
    featureStatus === "assigned" || featureStatus === "in-progress" || featureStatus === "review";
  if (runtime.runtimeState === "starting" && isActiveStatus) {
    return "live";
  }
  return runtime.runtimeState;
}

function mapPendingHandoff(
  entry: Awaited<ReturnType<typeof checkStatus>>["pendingHandoffs"][number],
): MissionControlHomeHandoff {
  return {
    id: entry.handoff.id,
    message: entry.handoff.message,
    agent: entry.handoff.session.agent,
    sessionId: entry.handoff.session.sessionId,
    sitrep: entry.handoff.sitrep,
    quickstart: entry.handoff.quickstart,
  };
}

async function buildMissionControlEnvironmentSummary(
  handoffStore: HandoffStorePort,
  config: ConfigPort,
  cass: CassPort,
  git: GitPort,
  cwd: string,
): Promise<{ status: StatusReport; checks: readonly DoctorCheck[] }> {
  const [
    pendingHandoffs,
    projectConfigExists,
    globalConfigExists,
    cassBinaryAvailable,
    gitAvailable,
  ] = await Promise.all([
    handoffStore.list({ status: "pending" }),
    config.exists("project", cwd),
    config.exists("global", cwd),
    cass.hasBinary(),
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
      pendingHandoffs,
      cassAvailable: cassBinaryAvailable,
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
        name: "cass",
        status: cassBinaryAvailable ? "ok" : "fail",
        message: cassBinaryAvailable ? "CASS binary detected" : "CASS binary not found",
        fix: cassBinaryAvailable ? undefined : CASS_INSTALL_HINT,
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
