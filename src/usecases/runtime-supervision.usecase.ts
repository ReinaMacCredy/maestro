import {
  DEFAULT_RUNTIME_FAILURE_MS,
  DEFAULT_RUNTIME_STALE_MS,
} from "../domain/defaults.js";
import type { RuntimeState, WorkerRuntime } from "../domain/runtime-types.js";

export interface RuntimeClassification {
  readonly runtimeState: RuntimeState;
  readonly lastSeenAgeMs: number;
  readonly startedAtMs: number;
}

export function classifyRuntime(
  runtime: WorkerRuntime,
  nowMs: number,
): RuntimeClassification {
  const lastSeenMs = new Date(runtime.lastSeenAt).getTime();
  const startedAtMs = new Date(runtime.startedAt).getTime();
  const lastSeenAgeMs = Math.max(0, nowMs - lastSeenMs);

  let runtimeState = runtime.runtimeState;
  if (runtimeState === "completed" || runtimeState === "recoverable" || runtimeState === "failed") {
    return {
      runtimeState,
      lastSeenAgeMs,
      startedAtMs,
    };
  }

  if (lastSeenAgeMs >= DEFAULT_RUNTIME_FAILURE_MS) {
    runtimeState = "failed";
  } else if (lastSeenAgeMs >= DEFAULT_RUNTIME_STALE_MS) {
    runtimeState = "stale";
  } else if (runtimeState === "stale") {
    runtimeState = "stale";
  } else if (runtimeState === "starting") {
    runtimeState = "starting";
  } else {
    runtimeState = "live";
  }

  return {
    runtimeState,
    lastSeenAgeMs,
    startedAtMs,
  };
}
