import { randomBytes } from "node:crypto";

/**
 * Evidence ID format: `evd-<13-digit ms timestamp>-<6 hex chars>`
 * (e.g. `evd-1714747200123-a1b2c3`).
 *
 * Fixed-width decimal timestamp keeps lexical sort aligned with chronological
 * sort up to year ~5138. 6 hex (24 bits) under the same millisecond gives
 * comfortably low collision odds at the 10-parallel-write bar set by the
 * roadmap, so no retry-on-collision logic is needed at this scale.
 */
export const EVIDENCE_ID_PATTERN = /^evd-\d{13}-[0-9a-f]{6}$/;

export function generateEvidenceId(now: () => number = Date.now): string {
  const ts = String(now()).padStart(13, "0");
  return `evd-${ts}-${randomBytes(3).toString("hex")}`;
}

export function isEvidenceId(value: string): boolean {
  return EVIDENCE_ID_PATTERN.test(value);
}
