import { FsVerdictStoreAdapter } from "./adapters/fs-verdict-store.adapter.js";
import type { VerdictStorePort } from "./ports/storage.js";

export interface VerdictServices {
  readonly verdictStore: VerdictStorePort;
}

export function buildVerdictServices(projectDir: string): VerdictServices {
  return {
    verdictStore: new FsVerdictStoreAdapter(projectDir),
  };
}
