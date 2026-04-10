/**
 * Keyword extractor for active-memory candidate matching.
 *
 * Phase 1 uses a deliberately dumb algorithm: lowercase, split on whitespace
 * and punctuation, drop stop words, drop tokens shorter than MIN_LENGTH,
 * drop pure numbers, dedupe while preserving order. No stemming, no
 * embeddings, no n-grams. Good enough for a pool of a few hundred
 * candidates at the scale maestro actually runs at. If the signal-to-noise
 * ever hurts, swap this one file for something smarter without changing
 * the contract.
 */

const STOP_WORDS: ReadonlySet<string> = new Set([
  "the", "and", "but", "for", "with", "was", "were", "been", "being",
  "have", "has", "had", "did", "does", "doing", "will", "would", "could",
  "should", "may", "might", "must", "shall", "can", "need", "into",
  "through", "during", "before", "after", "above", "below", "between",
  "out", "off", "over", "under", "again", "further", "then", "once",
  "this", "that", "these", "those", "its", "their", "them", "they",
  "what", "which", "who", "whom", "whose", "when", "where", "why", "how",
  "all", "any", "both", "each", "few", "more", "most", "other", "some",
  "such", "nor", "not", "only", "own", "same", "too", "very", "just",
  "also", "than", "from",
]);

const MIN_KEYWORD_LENGTH = 3;

export function extractKeywords(text: string): readonly string[] {
  if (typeof text !== "string" || text.length === 0) return [];

  const rawTokens = text.toLowerCase().split(/[\s\W_]+/);

  const seen = new Set<string>();
  const result: string[] = [];
  for (const token of rawTokens) {
    if (token.length < MIN_KEYWORD_LENGTH) continue;
    if (STOP_WORDS.has(token)) continue;
    if (/^\d+$/.test(token)) continue;
    if (seen.has(token)) continue;
    seen.add(token);
    result.push(token);
  }
  return result;
}
