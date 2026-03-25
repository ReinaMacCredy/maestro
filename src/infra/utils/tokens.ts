/**
 * Token estimation utility for DCP budget management.
 *
 * Uses chars/4 rule of thumb -- accurate within ~15% for English prose.
 * Code-heavy content may underestimate by 10-15% (more symbols = more tokens).
 * Uses text.length (UTF-16 code units), not byte length.
 */

/** Estimate token count from text content. */
export function estimateTokens(text: string): number {
  if (!text) return 0;
  return Math.ceil(text.length / 4);
}

/** Convert token budget to approximate byte budget for backward compat. */
export function tokensToBytesApprox(tokens: number): number {
  return tokens * 4;
}
