import type { RuntimeEventRecord } from "../domain/worker-types.js";

export interface RuntimeEventStorePort {
  append(missionId: string, event: RuntimeEventRecord): Promise<RuntimeEventRecord>;
  listByFeature(missionId: string, featureId: string): Promise<readonly RuntimeEventRecord[]>;
}
