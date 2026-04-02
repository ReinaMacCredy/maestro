import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { ExecutionStorePort } from "../ports/execution-store.port.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";
import type { TransportPort } from "../ports/transport.port.js";
import type { Feature, WorkerReport } from "../domain/mission-types.js";
import type { MaestroConfig } from "../domain/types.js";
import type { ExecutionRecord, FailureClass, WorkerConfig, WorkerResult } from "../domain/worker-types.js";
import { MaestroError } from "../domain/errors.js";
import { UNKNOWN_AGENT } from "../domain/defaults.js";
import { classifyRuntime } from "./runtime-supervision.usecase.js";
import { generateWorkerPrompt } from "./generate-worker-prompt.usecase.js";
import { parseWorkerReport, updateFeature } from "./feature-lifecycle.usecase.js";
import { selectWorker } from "./worker-selection.usecase.js";

export interface RunFeaturesOptions {
  readonly missionId: string;
  readonly featureIds?: readonly string[];
  readonly workerOverride?: string;
  readonly dryRun?: boolean;
}

export interface RunFeatureOutcome {
  readonly featureId: string;
  readonly title: string;
  readonly worker: string;
  readonly status: "dry-run" | "done" | "blocked" | "skipped";
  readonly summary: string;
  readonly durationMs?: number;
  readonly filesChanged?: readonly string[];
  readonly executionId?: string;
}

export interface RunFeaturesResult {
  readonly missionId: string;
  readonly dryRun: boolean;
  readonly success: boolean;
  readonly outcomes: readonly RunFeatureOutcome[];
  readonly stoppedOnFeatureId?: string;
}

export interface RunFeaturesDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly runtimeStore: RuntimeStorePort;
  readonly executionStore: ExecutionStorePort;
  readonly transport: TransportPort;
  readonly baseDir: string;
  readonly config: MaestroConfig;
}

export async function runFeatures(
  deps: RunFeaturesDeps,
  opts: RunFeaturesOptions,
): Promise<RunFeaturesResult> {
  const mission = await deps.missionStore.get(opts.missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${opts.missionId} not found`, [
      "List missions: maestro mission list",
    ]);
  }

  const allFeatures = await deps.featureStore.list(opts.missionId);
  const featureById = new Map(allFeatures.map((feature) => [feature.id, feature]));
  const orderedFeatures = mission.features
    .map((id) => featureById.get(id))
    .filter((feature): feature is Feature => feature !== undefined);

  const selectedFeatures = opts.featureIds && opts.featureIds.length > 0
    ? orderedFeatures.filter((feature) => opts.featureIds?.includes(feature.id))
    : orderedFeatures;

  if (selectedFeatures.length === 0) {
    throw new MaestroError("No matching features found to run", [
      "Use `maestro feature list --mission <id>` to inspect available features",
    ]);
  }

  const outcomes: RunFeatureOutcome[] = [];

  for (const feature of selectedFeatures) {
    if (feature.status === "done" || feature.status === "blocked") {
      outcomes.push({
        featureId: feature.id,
        title: feature.title,
        worker: opts.workerOverride ?? deps.config.execution?.defaultWorker ?? UNKNOWN_AGENT,
        status: "skipped",
        summary: `Feature is already ${feature.status}`,
      });
      continue;
    }

    if (!areDependenciesSatisfied(feature, featureById)) {
      outcomes.push({
        featureId: feature.id,
        title: feature.title,
        worker: opts.workerOverride ?? deps.config.execution?.defaultWorker ?? UNKNOWN_AGENT,
        status: "skipped",
        summary: "Dependencies are not satisfied",
      });
      continue;
    }

    const outcome = await runFeatureAttempt(deps, feature, opts);
    outcomes.push(outcome);
    const updatedFeature = await deps.featureStore.get(opts.missionId, feature.id);
    if (updatedFeature) {
      featureById.set(feature.id, updatedFeature);
    }

    if (outcome.status === "blocked" && deps.config.execution?.stopOnFailure !== false) {
      return {
        missionId: opts.missionId,
        dryRun: opts.dryRun === true,
        success: false,
        outcomes,
        stoppedOnFeatureId: feature.id,
      };
    }
  }

  return {
    missionId: opts.missionId,
    dryRun: opts.dryRun === true,
    success: outcomes.every((outcome) => outcome.status !== "blocked"),
    outcomes,
  };
}

export async function runFeatureAttempt(
  deps: RunFeaturesDeps,
  feature: Feature,
  opts: RunFeaturesOptions,
): Promise<RunFeatureOutcome> {
  const currentRuntime = await deps.runtimeStore.get(feature.missionId, feature.id);
  if (currentRuntime) {
    const classification = classifyRuntime(currentRuntime, Date.now());
    if (classification.runtimeState === "live" || classification.runtimeState === "starting") {
      throw new MaestroError(`Feature ${feature.id} already has live runtime ownership`, [
        "Wait for the current run to finish before starting a new one",
      ]);
    }
  }

  const history = await deps.executionStore.getByFeature(feature.missionId, feature.id);
  const workerSelection = opts.workerOverride
    ? resolveWorkerOverride(deps.config, opts.workerOverride)
    : selectWorker(deps.config, feature, history);
  const promptResult = await generateWorkerPrompt(
    deps.missionStore,
    deps.featureStore,
    deps.assertionStore,
    deps.runtimeStore,
    deps.baseDir,
    feature.missionId,
    feature.id,
  );

  await stampRuntimeAgent(deps.runtimeStore, feature.missionId, feature.id, workerSelection.slug);

  if (opts.dryRun) {
    return {
      featureId: feature.id,
      title: feature.title,
      worker: workerSelection.slug,
      status: "dry-run",
      summary: `Prompt generated (${promptResult.prompt.length} chars)`,
    };
  }

  if (feature.status === "pending") {
    await updateFeature(
      deps.missionStore,
      deps.featureStore,
      deps.runtimeStore,
      deps.baseDir,
      feature.missionId,
      feature.id,
      { status: "assigned" },
    );
  }

  await updateFeature(
    deps.missionStore,
    deps.featureStore,
    deps.runtimeStore,
    deps.baseDir,
    feature.missionId,
    feature.id,
    { status: "in-progress" },
  );

  const workerResult = await deps.transport.spawn(workerSelection.config, promptResult.prompt, {
    cwd: deps.baseDir,
    featureId: feature.id,
    missionId: feature.missionId,
    workerSlug: workerSelection.slug,
  });
  const report = await resolveWorkerReport(workerResult);

  await updateFeature(
    deps.missionStore,
    deps.featureStore,
    deps.runtimeStore,
    deps.baseDir,
    feature.missionId,
    feature.id,
    { status: "review", report },
  );

  await updateFeature(
    deps.missionStore,
    deps.featureStore,
    deps.runtimeStore,
    deps.baseDir,
    feature.missionId,
    feature.id,
    {
      status: workerResult.success ? "done" : "blocked",
      report,
    },
  );

  const runtime = await deps.runtimeStore.get(feature.missionId, feature.id);
  const startedAt = runtime?.startedAt ?? new Date(Date.now() - workerResult.durationMs).toISOString();
  const completedAt = new Date().toISOString();
  const executionId = runtime?.attemptId ?? crypto.randomUUID();

  const record: ExecutionRecord = {
    id: executionId,
    missionId: feature.missionId,
    featureId: feature.id,
    worker: workerSelection.slug,
    transport: workerSelection.config.transport,
    attemptId: executionId,
    startedAt,
    completedAt,
    durationMs: workerResult.durationMs,
    success: workerResult.success,
    exitCode: workerResult.exitCode,
    summary: workerResult.summary,
    stdoutRaw: workerResult.stdoutRaw,
    stderrRaw: workerResult.stderrRaw,
    filesChanged: workerResult.filesChanged,
    report,
    failureClass: workerResult.failureClass,
  };
  await deps.executionStore.save(feature.missionId, record);

  return {
    featureId: feature.id,
    title: feature.title,
    worker: workerSelection.slug,
    status: workerResult.success ? "done" : "blocked",
    summary: workerResult.summary,
    durationMs: workerResult.durationMs,
    filesChanged: workerResult.filesChanged,
    executionId,
  };
}

function areDependenciesSatisfied(
  feature: Feature,
  featureById: ReadonlyMap<string, Feature>,
): boolean {
  return feature.dependsOn.every((dependencyId) => featureById.get(dependencyId)?.status === "done");
}

function resolveWorkerOverride(config: MaestroConfig, workerSlug: string): { slug: string; config: WorkerConfig } {
  const workerConfig = config.workers?.[workerSlug];
  if (!workerConfig || !workerConfig.enabled) {
    throw new MaestroError(`Worker override '${workerSlug}' is not enabled`, [
      "Update config.workers to enable the requested worker",
    ]);
  }

  return { slug: workerSlug, config: workerConfig };
}

async function stampRuntimeAgent(
  runtimeStore: RuntimeStorePort,
  missionId: string,
  featureId: string,
  workerSlug: string,
): Promise<void> {
  const runtime = await runtimeStore.get(missionId, featureId);
  if (!runtime) return;

  await runtimeStore.save(missionId, featureId, {
    ...runtime,
    agent: workerSlug,
  });
}

async function resolveWorkerReport(result: WorkerResult): Promise<WorkerReport> {
  const parsedValue = result.parsedOutput?.trim() || result.stdoutRaw.trim();
  if (parsedValue.length > 0) {
    try {
      return await parseWorkerReport(parsedValue);
    } catch {
      // Fall through to the synthesized report below.
    }
  }

  return buildSyntheticReport(result);
}

function buildSyntheticReport(result: WorkerResult): WorkerReport {
  const discoveredIssues = result.success
    ? []
    : [{
        severity: failureSeverity(result.failureClass),
        description: result.summary,
        suggestedFix: result.stderrRaw || undefined,
      }];

  return {
    salientSummary: result.summary,
    whatWasImplemented: result.success ? result.summary : "",
    whatWasLeftUndone: result.success ? "" : result.summary,
    verification: {
      commandsRun: [],
      interactiveChecks: [],
    },
    tests: {
      added: [],
    },
    discoveredIssues,
  };
}

function failureSeverity(failureClass: FailureClass | undefined): string {
  switch (failureClass) {
    case "infrastructure":
      return "high";
    case "validation":
      return "medium";
    case "worker-crash":
    case "unknown":
    default:
      return "medium";
  }
}
