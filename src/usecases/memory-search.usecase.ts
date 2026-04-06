import type { Correction, RawLearningEntry } from "../domain/memory-types.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";

export interface SearchResult {
  readonly corrections: readonly Correction[];
  readonly learnings: readonly RawLearningEntry[];
}

export async function searchMemory(
  corrStore: CorrectionStorePort,
  learnStore: LearningStorePort,
  query: string,
): Promise<SearchResult> {
  const corrections = await corrStore.search({ text: query });

  const allLearnings = await learnStore.listRaw();
  const lower = query.toLowerCase();
  const learnings = allLearnings.filter((l) =>
    l.content.toLowerCase().includes(lower),
  );

  return { corrections, learnings };
}
