import type { ExecutionRecord } from "../domain/worker-types.js";

export interface ExecutionStorePort {
  get(missionId: string, executionId: string): Promise<ExecutionRecord | undefined>;
  save(missionId: string, record: ExecutionRecord): Promise<ExecutionRecord>;
  list(missionId: string): Promise<readonly ExecutionRecord[]>;
  getByFeature(missionId: string, featureId: string): Promise<readonly ExecutionRecord[]>;
}
