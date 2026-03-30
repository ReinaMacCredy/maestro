/**
 * Formatting helpers for TUI display values.
 */

/** Format elapsed milliseconds as human-readable duration. */
export function formatElapsed(ms: number): string {
  if (ms < 0) ms = 0;
  const totalSeconds = Math.floor(ms / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) {
    return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  const remainMin = minutes % 60;
  return remainMin > 0 ? `${hours}h ${remainMin}m` : `${hours}h`;
}

/** Format token count for compact display. null = "--". */
export function formatTokens(n: number | null): string {
  if (n === null || n === undefined) return "--";
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** Format a timestamp as relative time (HH:MM) from a base timestamp. */
export function formatRelativeTime(timestampMs: number, baseMs: number): string {
  const delta = Math.max(0, timestampMs - baseMs);
  const totalMinutes = Math.floor(delta / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

/** Truncate text to maxLen, appending ellipsis if truncated. */
export function truncate(text: string, maxLen: number): string {
  if (maxLen <= 0) return "";
  if (text.length <= maxLen) return text;
  if (maxLen <= 3) return text.slice(0, maxLen);
  return text.slice(0, maxLen - 3) + "...";
}
