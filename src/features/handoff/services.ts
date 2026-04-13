import type { HandoffStorePort } from "./ports/handoff-store.port.js";
import { FsHandoffStoreAdapter } from "./adapters/handoff-store.adapter.js";

export interface HandoffServices {
  readonly handoffStore: HandoffStorePort;
}

export function buildHandoffServices(projectDir: string): HandoffServices {
  return {
    handoffStore: new FsHandoffStoreAdapter(projectDir),
  };
}
