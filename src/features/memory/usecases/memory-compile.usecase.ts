import type { CompiledLearnings, RawLearningEntry } from "../domain/memory-types.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";

export interface CompileResult {
  readonly compiled: CompiledLearnings;
  readonly rawEntries: readonly RawLearningEntry[];
}

export async function compileLearnings(
  store: LearningStorePort,
  summary: string,
): Promise<CompileResult> {
  const rawEntries = await store.listRaw();
  const compiled: CompiledLearnings = {
    compiledAt: new Date().toISOString(),
    summary,
    rawCount: rawEntries.length,
  };
  await store.writeCompiled(compiled);
  return { compiled, rawEntries };
}
