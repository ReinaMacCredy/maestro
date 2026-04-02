import { DEFAULT_RUNTIME_LEASE_MS } from "../domain/defaults.js";
import type { WorkerRuntime } from "../domain/runtime-types.js";
import type { RuntimeEventRecord, WorkerProgressEvent } from "../domain/worker-types.js";
import type { RuntimeEventStorePort } from "../ports/runtime-event-store.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";

export function applyWorkerProgressEvent(
  runtime: WorkerRuntime,
  event: WorkerProgressEvent,
): WorkerRuntime {
  const nextTimestampMs = new Date(event.timestamp).getTime();
  const nextRuntimeState = event.runtimeState
    ?? (event.kind === "status" && event.text ? runtime.runtimeState : "live");

  return {
    ...runtime,
    agent: event.worker || runtime.agent,
    sessionId: event.sessionId ?? runtime.sessionId,
    runtimeState: nextRuntimeState,
    lastSeenAt: event.timestamp,
    leaseExpiresAt: new Date(nextTimestampMs + DEFAULT_RUNTIME_LEASE_MS).toISOString(),
    failureReason: nextRuntimeState === "failed"
      ? event.text ?? runtime.failureReason
      : runtime.failureReason,
  };
}

export async function recordWorkerProgressEvent(
  runtimeStore: RuntimeStorePort,
  runtimeEventStore: RuntimeEventStorePort,
  missionId: string,
  featureId: string,
  event: WorkerProgressEvent,
): Promise<void> {
  const runtime = await runtimeStore.get(missionId, featureId);
  if (!runtime) {
    return;
  }

  const updatedRuntime = applyWorkerProgressEvent(runtime, event);
  await runtimeStore.save(missionId, featureId, updatedRuntime);

  const record: RuntimeEventRecord = {
    id: crypto.randomUUID(),
    missionId,
    featureId,
    attemptId: runtime.attemptId,
    worker: event.worker,
    timestamp: event.timestamp,
    kind: event.kind,
    text: event.text,
    sessionId: event.sessionId,
    runtimeState: event.runtimeState,
  };
  await runtimeEventStore.append(missionId, record);
}
