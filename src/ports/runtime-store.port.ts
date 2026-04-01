import type { WorkerRuntime } from "../domain/runtime-types.js";

export interface RuntimeStorePort {
  get(missionId: string, featureId: string): Promise<WorkerRuntime | undefined>;
  save(missionId: string, featureId: string, runtime: WorkerRuntime): Promise<WorkerRuntime>;
  delete(missionId: string, featureId: string): Promise<boolean>;
  list(missionId: string): Promise<readonly WorkerRuntime[]>;
}
