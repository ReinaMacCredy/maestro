/**
 * Deliberately simple keyword extractor: lowercase, split on whitespace and
 * punctuation, drop stop words, drop short or numeric tokens, dedupe. No
 * stemming or embeddings — swap this file wholesale if signal-to-noise ever
 * hurts; callers depend only on the exported `extractKeywords` signature.
 */

const TOKENIZER = /[\s\W_]+/;

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

  const rawTokens = text.toLowerCase().split(TOKENIZER);

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
