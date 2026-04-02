import type { Feature } from "../domain/mission-types.js";
import type { ExecutionRecord, WorkerConfig } from "../domain/worker-types.js";
import type { MaestroConfig } from "../domain/types.js";
import { MaestroError } from "../domain/errors.js";

export interface SelectWorkerResult {
  readonly slug: string;
  readonly config: WorkerConfig;
}

export function selectWorker(
  config: MaestroConfig,
  _feature: Feature,
  previousAttempts: readonly ExecutionRecord[] = [],
): SelectWorkerResult {
  const workers = Object.entries(config.workers ?? {})
    .filter(([, workerConfig]) => workerConfig.enabled);

  if (workers.length === 0) {
    throw new MaestroError("No enabled workers are configured", [
      "Add an enabled worker profile under config.workers",
    ]);
  }

  const defaultWorker = config.execution?.defaultWorker ?? workers[0]?.[0];
  if (!defaultWorker) {
    throw new MaestroError("No default worker configured", [
      "Set execution.defaultWorker or enable at least one worker profile",
    ]);
  }

  const shouldRotate = config.execution?.rotateWorkerOnRetry === true && previousAttempts.length > 0;
  const lastWorker = previousAttempts.at(-1)?.worker;
  const orderedWorkers = shouldRotate
    ? workers.filter(([slug]) => slug !== lastWorker)
    : workers;

  const chosen = orderedWorkers.find(([slug]) => slug === defaultWorker)
    ?? orderedWorkers[0]
    ?? workers.find(([slug]) => slug === defaultWorker)
    ?? workers[0];

  if (!chosen) {
    throw new MaestroError("Could not select a worker profile");
  }

  return {
    slug: chosen[0],
    config: chosen[1],
  };
}
