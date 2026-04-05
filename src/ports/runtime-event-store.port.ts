import type { RuntimeEventRecord } from "../domain/worker-types.js";

export interface RuntimeEventTailOptions {
  readonly maxBytes?: number;
  readonly maxLines?: number;
}

export interface RuntimeEventStorePort {
  append(missionId: string, event: RuntimeEventRecord): Promise<RuntimeEventRecord>;
  listByFeature(missionId: string, featureId: string): Promise<readonly RuntimeEventRecord[]>;
  tailByFeature?(
    missionId: string,
    featureId: string,
    options?: RuntimeEventTailOptions,
  ): Promise<readonly RuntimeEventRecord[]>;
}
