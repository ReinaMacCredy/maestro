/**
 * Pure text utility functions for keyword extraction and set comparison.
 * Domain-layer -- no app or infra dependencies.
 */

export const STOPWORDS = new Set([
  'the', 'and', 'for', 'that', 'this', 'with', 'from', 'have', 'will', 'are',
  'was', 'been', 'not', 'but', 'can', 'all', 'its', 'also', 'into', 'when',
  'then', 'than', 'each', 'such', 'only', 'some', 'just', 'more', 'most',
  'very', 'much', 'your', 'what', 'which', 'they', 'them', 'their', 'there',
  'here', 'where', 'about', 'after', 'before', 'other',
]);

/** Doctrine relevance scoring weights (tag overlap vs keyword overlap). */
export const TAG_WEIGHT = 0.6;
export const KEYWORD_WEIGHT = 0.4;

/**
 * Extract meaningful words from text for keyword matching.
 * Lowercase, split on whitespace/punctuation, remove stopwords, filter < 4 chars.
 */
export function extractKeywords(text: string): Set<string> {
  const words = text
    .toLowerCase()
    .split(/[\s\-_,.;:!?()[\]{}"'`\/\\|#@&=+*<>]+/)
    .filter(w => w.length >= 4 && !STOPWORDS.has(w));
  return new Set(words);
}

/**
 * Compute overlap between two keyword sets.
 * 'simpson' = Szymkiewicz-Simpson coefficient (intersection / min) -- sensitive to subset relationships.
 * 'jaccard' = Jaccard index (intersection / union) -- stricter, ignores set size imbalance.
 */
export function computeSetOverlap(a: Set<string>, b: Set<string>, mode: 'jaccard' | 'simpson' = 'simpson'): number {
  if (a.size === 0 || b.size === 0) return 0;
  let intersection = 0;
  const smaller = a.size <= b.size ? a : b;
  const larger = a.size <= b.size ? b : a;
  for (const word of smaller) {
    if (larger.has(word)) intersection++;
  }
  if (mode === 'simpson') return intersection / Math.min(a.size, b.size);
  const union = a.size + b.size - intersection;
  return union > 0 ? intersection / union : 0;
}

export function scorePriority(priority: number | undefined): number {
  const clamped = Math.max(0, Math.min(4, priority ?? 2));
  return (4 - clamped) / 4;
}
