/**
 * Cap `value` to at most `max` characters, replacing the trailing character
 * with a U+2026 ellipsis when truncation occurs. The single-char ellipsis
 * suits agent-facing JSON payloads where byte budget is the constraint;
 * TUI text uses the three-dot `"..."` form (see `src/tui/shared/format.ts`).
 */
export function truncateText(value: string, max: number): string {
  if (value.length <= max) return value;
  return value.slice(0, max - 1).trimEnd() + "…";
}

const FIRST_SENTENCE_RE = /^.*?[.!?](?=\s|$)/;

export function firstSentence(value: string): string {
  return value.match(FIRST_SENTENCE_RE)?.[0] ?? value;
}
