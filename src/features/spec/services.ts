import type { SpecStorePort } from "./ports/storage.js";
import { FsSpecStoreAdapter } from "./adapters/fs-spec-store.adapter.js";

export interface SpecServices {
  readonly specStore: SpecStorePort;
}

export function buildSpecServices(projectDir: string): SpecServices {
  return {
    specStore: new FsSpecStoreAdapter(projectDir),
  };
}
