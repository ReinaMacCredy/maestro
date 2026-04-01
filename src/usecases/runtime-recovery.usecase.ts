import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";
import { DEFAULT_RUNTIME_RETRY_BUDGET } from "../domain/defaults.js";
import { MaestroError } from "../domain/errors.js";
import type { Feature } from "../domain/mission-types.js";
import type { RecoveryHistoryEntry, WorkerRuntime } from "../domain/runtime-types.js";
import { classifyRuntime } from "./runtime-supervision.usecase.js";

export interface RecoverRuntimeFailureResult {
  readonly recovered: boolean;
  readonly exhausted: boolean;
  readonly feature?: Feature;
  readonly runtime?: WorkerRuntime;
}

export interface RecoverMissionRuntimeFailuresResult {
  readonly recovered: readonly RecoverRuntimeFailureResult[];
}

export async function recoverMissionRuntimeFailures(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  runtimeStore: RuntimeStorePort,
  missionId: string,
  nowMs = Date.now(),
): Promise<RecoverMissionRuntimeFailuresResult> {
  const runtimes = await runtimeStore.list(missionId);
  const recovered: RecoverRuntimeFailureResult[] = [];

  for (const runtime of runtimes) {
    const feature = await featureStore.get(missionId, runtime.featureId);
    if (!feature) {
      continue;
    }

    const result = await recoverRuntimeFailure(
      missionStore,
      featureStore,
      runtimeStore,
      missionId,
      runtime.featureId,
      nowMs,
    );
    if (result.recovered || result.exhausted) {
      recovered.push(result);
    }
  }

  return { recovered };
}

export async function recoverRuntimeFailure(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  runtimeStore: RuntimeStorePort,
  missionId: string,
  featureId: string,
  nowMs = Date.now(),
): Promise<RecoverRuntimeFailureResult> {
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`);
  }

  const feature = await featureStore.get(missionId, featureId);
  if (!feature) {
    throw new MaestroError(`Feature ${featureId} not found in mission ${missionId}`);
  }

  const runtime = await runtimeStore.get(missionId, featureId);
  if (!runtime) {
    return { recovered: false, exhausted: false, feature };
  }

  const classification = classifyRuntime(runtime, nowMs);
  const isActiveFeature = feature.status === "assigned" || feature.status === "in-progress";
  if (!isActiveFeature || classification.runtimeState !== "failed") {
    return { recovered: false, exhausted: false, feature, runtime };
  }

  const nowIso = new Date(nowMs).toISOString();
  const reason = runtime.failureReason ?? `Runtime heartbeat expired after ${Math.round(classification.lastSeenAgeMs / 1000)}s`;
  const exhausted = runtime.recoveryMetadata.retryCount >= DEFAULT_RUNTIME_RETRY_BUDGET;

  if (exhausted) {
    if (hasRecordedExhaustedFailure(runtime)) {
      return { recovered: false, exhausted: true, feature, runtime };
    }

    const exhaustedRuntime: WorkerRuntime = {
      ...runtime,
      runtimeState: "failed",
      failureReason: reason,
      recoveryMetadata: {
        retryCount: runtime.recoveryMetadata.retryCount,
        lastRecoveryAt: nowIso,
        lastRecoveryReason: `${reason} (retry budget exhausted)`,
        history: [
          ...runtime.recoveryMetadata.history,
          makeRecoveryHistory(nowIso, reason, "failed", "failed"),
        ],
      },
    };
    await runtimeStore.save(missionId, featureId, exhaustedRuntime);
    return { recovered: false, exhausted: true, feature, runtime: exhaustedRuntime };
  }

  const updatedFeature = await featureStore.update(missionId, featureId, { status: "pending" });
  if (!updatedFeature) {
    throw new MaestroError(`Failed to update feature ${featureId}`);
  }

  const recoveredRuntime: WorkerRuntime = {
    ...runtime,
    runtimeState: "recoverable",
    failureReason: reason,
    recoveryMetadata: {
      retryCount: runtime.recoveryMetadata.retryCount + 1,
      lastRecoveryAt: nowIso,
      lastRecoveryReason: reason,
      history: [
        ...runtime.recoveryMetadata.history,
        makeRecoveryHistory(nowIso, reason, "failed", "recoverable"),
      ],
    },
  };
  await runtimeStore.save(missionId, featureId, recoveredRuntime);

  return {
    recovered: true,
    exhausted: false,
    feature: updatedFeature,
    runtime: recoveredRuntime,
  };
}

function hasRecordedExhaustedFailure(runtime: WorkerRuntime): boolean {
  const lastHistoryEntry = runtime.recoveryMetadata.history.at(-1);
  return runtime.recoveryMetadata.lastRecoveryReason?.endsWith("(retry budget exhausted)") === true
    && lastHistoryEntry?.fromState === "failed"
    && lastHistoryEntry?.toState === "failed";
}

function makeRecoveryHistory(
  timestamp: string,
  reason: string,
  fromState: WorkerRuntime["runtimeState"],
  toState: WorkerRuntime["runtimeState"],
): RecoveryHistoryEntry {
  return {
    timestamp,
    reason,
    fromState,
    toState,
  };
}
