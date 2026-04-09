import type { RawLearningEntry, CompiledLearnings } from "../domain/memory-types.js";

export interface LearningStorePort {
  appendRaw(entry: RawLearningEntry): Promise<void>;
  listRaw(): Promise<readonly RawLearningEntry[]>;
  rawCount(): Promise<number>;
  readCompiled(): Promise<CompiledLearnings | undefined>;
  writeCompiled(compiled: CompiledLearnings): Promise<void>;
}
