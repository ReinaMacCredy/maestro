import type { RuntimeEventRecord } from "../domain/worker-types.js";

export const DEFAULT_RUNTIME_EVENT_TAIL_MAX_BYTES = 512 * 1024;
export const DEFAULT_RUNTIME_EVENT_TAIL_MAX_LINES = 256;

export interface RuntimeEventTailOptions {
  readonly maxBytes?: number;
  readonly maxLines?: number;
}

export interface RuntimeEventStorePort {
  append(missionId: string, event: RuntimeEventRecord): Promise<RuntimeEventRecord>;
  listByFeature(missionId: string, featureId: string): Promise<readonly RuntimeEventRecord[]>;
  tailByFeature(
    missionId: string,
    featureId: string,
    options?: RuntimeEventTailOptions,
  ): Promise<readonly RuntimeEventRecord[]>;
}
