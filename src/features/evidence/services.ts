import type { EvidenceStorePort } from "./ports/storage.js";
import { FsEvidenceStoreAdapter } from "./adapters/file-storage.js";

export interface EvidenceServices {
  readonly evidenceStore: EvidenceStorePort;
}

export function buildEvidenceServices(projectDir: string): EvidenceServices {
  return {
    evidenceStore: new FsEvidenceStoreAdapter(projectDir),
  };
}
