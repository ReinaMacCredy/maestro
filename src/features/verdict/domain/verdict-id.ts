import { randomBytes } from "node:crypto";

/**
 * Verdict ID format: `vrd-<13-digit ms timestamp>-<6 hex chars>`
 * (e.g. `vrd-1714747200123-a1b2c3`).
 *
 * Fixed-width decimal timestamp keeps lexical sort aligned with chronological
 * sort up to year ~5138. 6 hex (24 bits) under the same millisecond gives
 * comfortably low collision odds at the 10-parallel-write bar set by the
 * roadmap, so no retry-on-collision logic is needed at this scale.
 */
export const VERDICT_ID_PATTERN = /^vrd-\d{13}-[0-9a-f]{6}$/;

export function generateVerdictId(now: () => number = Date.now): string {
  const ts = String(now()).padStart(13, "0");
  return `vrd-${ts}-${randomBytes(3).toString("hex")}`;
}

export function isVerdictId(value: string): boolean {
  return VERDICT_ID_PATTERN.test(value);
}
