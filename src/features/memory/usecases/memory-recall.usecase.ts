import type { Correction, CompiledLearnings } from "../domain/memory-types.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";

export interface RecallContext {
  readonly taskDescription?: string;
  readonly filePaths?: readonly string[];
}

export interface RecallResult {
  readonly corrections: readonly Correction[];
  readonly compiledLearnings?: CompiledLearnings;
}

const STOP_WORDS = new Set([
  "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
  "have", "has", "had", "do", "does", "did", "will", "would", "could",
  "should", "may", "might", "shall", "can", "to", "of", "in", "for",
  "on", "with", "at", "by", "from", "as", "into", "through", "during",
  "before", "after", "and", "but", "or", "nor", "not", "so", "yet",
  "both", "either", "neither", "each", "every", "all", "any", "few",
  "more", "most", "other", "some", "such", "no", "only", "same", "than",
  "too", "very", "just", "because", "if", "when", "while", "this", "that",
  "these", "those", "it", "its",
]);

export async function recallMemory(
  corrStore: CorrectionStorePort,
  learnStore: LearningStorePort,
  ctx: RecallContext,
): Promise<RecallResult> {
  const all = await corrStore.list();
  const scored = all.map((c) => ({ correction: c, score: scoreCorrection(c, ctx) }));

  const hardAlways = scored.filter((s) => s.correction.severity === "hard" && s.score < 0.3);
  const matched = scored.filter((s) => s.score >= 0.3);

  const combined = [...matched, ...hardAlways]
    .sort((a, b) => b.score - a.score)
    .slice(0, 10)
    .map((s) => s.correction);

  const compiledLearnings = await learnStore.readCompiled();

  return { corrections: combined, compiledLearnings };
}

function scoreCorrection(correction: Correction, ctx: RecallContext): number {
  let keywordScore = 0;
  let fileGlobScore = 0;

  if (ctx.taskDescription && correction.trigger.keywords.length > 0) {
    const tokens = tokenize(ctx.taskDescription);
    const triggerKeywords = correction.trigger.keywords.map((k) => k.toLowerCase());
    let matches = 0;
    for (const kw of triggerKeywords) {
      if (tokens.some((t) => t.includes(kw) || kw.includes(t))) matches++;
    }
    keywordScore = matches / triggerKeywords.length;
  }

  if (ctx.filePaths?.length && correction.trigger.fileGlobs.length > 0) {
    for (const fp of ctx.filePaths) {
      for (const g of correction.trigger.fileGlobs) {
        const glob = new Bun.Glob(g);
        if (glob.match(fp)) {
          fileGlobScore = 1;
          break;
        }
      }
      if (fileGlobScore > 0) break;
    }
  }

  return keywordScore * 0.7 + fileGlobScore * 0.3;
}

function tokenize(text: string): string[] {
  return text
    .toLowerCase()
    .split(/\W+/)
    .filter((t) => t.length > 1 && !STOP_WORDS.has(t));
}
