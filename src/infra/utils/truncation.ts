/**
 * List truncation utilities for CLI output.
 */

export interface TruncateResult<T> {
  items: T[];
  truncated: number;
}

export function truncateList<T>(items: T[], max: number): TruncateResult<T> {
  if (items.length <= max) {
    return { items, truncated: 0 };
  }
  return { items: items.slice(0, max), truncated: items.length - max };
}

export function formatTruncation(truncated: number, label?: string): string {
  if (truncated === 0) return "";
  return `... and ${truncated} more ${label || "items"}`;
}
