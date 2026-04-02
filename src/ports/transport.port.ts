import type { WorkerConfig, WorkerProgressEvent, WorkerResult } from "../domain/worker-types.js";

export interface TransportPort {
  spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: {
      cwd: string;
      featureId: string;
      missionId: string;
      workerSlug: string;
      onEvent?: (event: WorkerProgressEvent) => void | Promise<void>;
    },
  ): Promise<WorkerResult>;
}
