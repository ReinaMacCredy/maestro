import type { RatchetStorePort } from "./ports/ratchet-store.port.js";
import { FsRatchetStoreAdapter } from "./adapters/ratchet-store.adapter.js";

export interface RatchetServices {
  readonly ratchetStore: RatchetStorePort;
}

export function buildRatchetServices(projectDir: string): RatchetServices {
  return {
    ratchetStore: new FsRatchetStoreAdapter(projectDir),
  };
}
