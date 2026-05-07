import type { RuntimeSignal } from "@/features/spec";
import type { RuntimeSignalResult } from "../domain/types.js";

export interface RuntimeMonitorPort {
  query(signal: RuntimeSignal): Promise<RuntimeSignalResult>;
}
