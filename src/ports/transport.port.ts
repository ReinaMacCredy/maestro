import type { WorkerConfig, WorkerProgressEvent, WorkerResult } from "../domain/worker-types.js";

export interface TransportSpawnOptions {
  readonly cwd: string;
  readonly featureId: string;
  readonly missionId: string;
  readonly workerSlug: string;
  readonly onEvent?: (event: WorkerProgressEvent) => void | Promise<void>;
}

export interface TransportPort {
  spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: TransportSpawnOptions,
  ): Promise<WorkerResult>;
}
