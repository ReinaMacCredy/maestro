/**
 * Build a MissionControlSnapshot from existing stores.
 * Polls once -- no subscriptions, no event tailing.
 */
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../ports/checkpoint-store.port.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";
import type { Mission, Feature } from "../domain/mission-types.js";
import type { RuntimeState, WorkerRuntime } from "../domain/runtime-types.js";
import type { DoctorCheck, StatusReport } from "../domain/types.js";
import { CASS_INSTALL_HINT } from "../domain/defaults.js";
import { generateMissionReport, type MissionReport } from "../usecases/mission-report.usecase.js";
import { checkStatus } from "../usecases/check-status.usecase.js";
import { runDoctor } from "../usecases/run-doctor.usecase.js";
import { getValidFeatureTransitions } from "../domain/mission-state.js";
import { classifyRuntime } from "../usecases/runtime-supervision.usecase.js";
import { recoverMissionRuntimeFailures } from "../usecases/runtime-recovery.usecase.js";
import { deriveEvents } from "./events.js";
import type {
  MissionControlSnapshot,
  MissionControlFeatureRow,
  MissionControlFeatureDetail,
  MissionControlWorkerPane,
  MissionControlMilestoneRow,
  MissionControlHomeAction,
  MissionControlHomeHandoff,
  MissionControlRuntimeProcessRow,
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
}

/**
 * Build a complete snapshot for the mission control dashboard.
 * Throws if mission not found.
 */
export async function buildSnapshot(
  deps: SnapshotDeps,
  missionId: string,
): Promise<MissionControlSnapshot> {
  await recoverMissionRuntimeFailures(
    deps.missionStore,
    deps.featureStore,
    deps.runtimeStore,
    missionId,
  );

  const [
    report,
    features,
    assertions,
    checkpoints,
    env,
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
    deps.git.getState(deps.cwd),
    deps.runtimeStore.list(missionId),
  ]);

  const mission = report.mission;
  const now = Date.now();
  const startMs = new Date(mission.approvedAt ?? mission.createdAt).getTime();
  const runtimeByFeature = new Map(
    runtimes.map((runtime) => [runtime.featureId, buildRuntimeView(runtime, now)]),
  );

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
  const activeFeature = findActiveFeature(features, report, runtimeByFeature);

  // Active worker
  const activeWorker = buildActiveWorker(features, runtimeByFeature, startMs, now);

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
  const blockedCount = features.filter((f) => f.status === "blocked").length;
  const queuedCount = features.filter((f) => f.status === "pending").length;
    const pendingHandoffs = env.status.pendingHandoffs.map(mapPendingHandoff);
    const workerTypes = [...new Set(features.map((feature) => feature.workerType))];

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
    activeFeature,
    features: featureRows,
    activeWorker,
    session: {
      branch: gitState.branch,
      workingTreeClean: gitState.workingTreeClean,
      diffStat: gitState.diffStat,
      changedFiles: gitState.changedFiles,
    },
      pendingHandoffs,
      configSummary: {
        configSource: env.status.configSource,
        cassAvailable: env.status.cassAvailable,
        gitAvailable: env.status.gitAvailable,
        checks: env.checks,
        missionDirectory: `.maestro/missions/${mission.id}`,
        workerTypes,
      },
    runtimeProcesses: buildRuntimeProcesses(features, runtimeByFeature, activeWorker),
    progressLog,
    milestones,
    canPause: mission.status === "executing",
    canResume: mission.status === "paused",
    home: null,
  };
}

export async function buildHomeSnapshot(
  deps: HomeSnapshotDeps,
  cwd: string,
): Promise<MissionControlSnapshot> {
  const [env, gitState] = await Promise.all([
    buildMissionControlEnvironmentSummary(deps.handoffStore, deps.config, deps.cass, deps.git, cwd),
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
    activeFeature: null,
    features: [],
    activeWorker: null,
    session: gitState
      ? {
        branch: gitState.branch,
        workingTreeClean: gitState.workingTreeClean,
        diffStat: gitState.diffStat,
        changedFiles: gitState.changedFiles,
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
    runtimeProcesses: [],
    progressLog: [],
    milestones: [],
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

function findActiveFeature(
  features: readonly Feature[],
  report: MissionReport,
  runtimeByFeature?: ReadonlyMap<string, RuntimeView>,
): MissionControlFeatureDetail | null {
  const active = features.find(
    (f) => f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  ) ?? features.find((f) => f.status === "pending");

  if (!active) return null;

  const milestone = report.mission.milestones.find((m) => m.id === active.milestoneId);
  const runtime = runtimeByFeature?.get(active.id);

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
      runtimeState: presentRuntimeState(runtime, active.status),
      lastSeenAgeMs: runtime?.lastSeenAgeMs,
      failureReason: runtime?.failureReason,
    retryCount: runtime?.retryCount,
    agent: runtime?.agent,
    sessionId: runtime?.sessionId,
  };
}

function buildActiveWorker(
  features: readonly Feature[],
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  startMs: number,
  nowMs: number,
): MissionControlWorkerPane | null {
  const active = features.find(
    (f) => f.status === "assigned" || f.status === "in-progress",
  );

  if (!active) return null;

  const runtime = runtimeByFeature.get(active.id);
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
  features: readonly Feature[],
  runtimeByFeature: ReadonlyMap<string, RuntimeView>,
  activeWorker: MissionControlWorkerPane | null,
): readonly MissionControlRuntimeProcessRow[] {
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
      return {
        featureId: feature.id,
        title: feature.title,
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
      };
    });
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
