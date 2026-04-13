import type { CorrectionStorePort } from "./ports/correction-store.port.js";
import type { LearningStorePort } from "./ports/learning-store.port.js";
import { FsCorrectionStoreAdapter } from "./adapters/correction-store.adapter.js";
import { FsLearningStoreAdapter } from "./adapters/learning-store.adapter.js";

export interface MemoryServices {
  readonly correctionStore: CorrectionStorePort;
  readonly learningStore: LearningStorePort;
}

export function buildMemoryServices(projectDir: string): MemoryServices {
  return {
    correctionStore: new FsCorrectionStoreAdapter(projectDir),
    learningStore: new FsLearningStoreAdapter(projectDir),
  };
}
