import type { RuntimeSignal } from "@/shared/domain/legacy-spec/index.js";
import type { RuntimeSignalResult } from "../domain/types.js";

export interface RuntimeMonitorPort {
  query(signal: RuntimeSignal): Promise<RuntimeSignalResult>;
}
