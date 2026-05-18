import type { EvidenceStorePort } from "./ports/storage.js";
import { FsEvidenceStoreAdapter } from "./adapters/file-storage.js";

export interface EvidenceServices {
  readonly legacyEvidenceStore: EvidenceStorePort;
}

export function buildEvidenceServices(projectDir: string): EvidenceServices {
  return {
    legacyEvidenceStore: new FsEvidenceStoreAdapter(projectDir),
  };
}
