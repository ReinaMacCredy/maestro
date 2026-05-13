/**
 * Token estimation for token-budget regression tracking.
 *
 * Anthropic does not ship a public offline BPE for Claude, and the plan
 * contract forbids adding a tokenizer dependency. We use a calibrated
 * char-ratio estimator that matches Anthropic's published "~4 chars per
 * token" rule of thumb (https://docs.anthropic.com/claude/docs/glossary).
 *
 * Empirically, JSON-shaped output tokenizes at ~3.5 chars/token because
 * structural characters split into more tokens; prose tokenizes at ~4.
 * The estimator picks the ratio per detected shape and rounds up.
 *
 * Use this for **regression tracking only** — comparing the same shape of
 * output before and after a change. Do not use it to make absolute
 * cost projections.
 */

const JSON_CHARS_PER_TOKEN = 3.5;
const PROSE_CHARS_PER_TOKEN = 4;

export type TokenShape = "json" | "prose";

export function estimateTokens(text: string, shape: TokenShape = "prose"): number {
  if (text.length === 0) return 0;
  const ratio = shape === "json" ? JSON_CHARS_PER_TOKEN : PROSE_CHARS_PER_TOKEN;
  return Math.ceil(text.length / ratio);
}

/**
 * Detect shape from the first non-whitespace character. JSON output starts
 * with `{` or `[`; anything else is treated as prose.
 */
export function detectShape(text: string): TokenShape {
  const trimmed = text.trimStart();
  return trimmed.startsWith("{") || trimmed.startsWith("[") ? "json" : "prose";
}

export function estimateTokensAuto(text: string): number {
  return estimateTokens(text, detectShape(text));
}
