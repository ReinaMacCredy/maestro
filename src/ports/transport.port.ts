import type { WorkerConfig, WorkerResult } from "../domain/worker-types.js";

export interface TransportPort {
  spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: {
      cwd: string;
      featureId: string;
      missionId: string;
      workerSlug: string;
    },
  ): Promise<WorkerResult>;
}
