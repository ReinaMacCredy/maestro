/**
 * Criterion ID format: `crt-<13-digit ms timestamp>-<8 hex chars>`
 *
 * The 13-digit decimal timestamp keeps lexical sort aligned with chronological
 * sort up to year ~5138. 8 hex (32 bits) gives very low collision odds when
 * criteria are created in bursts. Ids are stable: once written to storage they
 * must never change so the ProofMap can join on them reliably.
 */
import { randomBytes } from "node:crypto";

export const CRITERION_ID_PATTERN = /^crt-\d{13}-[0-9a-f]{8}$/;

export function generateCriterionId(now: () => number = Date.now): string {
  const ts = String(now()).padStart(13, "0");
  return `crt-${ts}-${randomBytes(4).toString("hex")}`;
}

export function isCriterionId(value: string): boolean {
  return CRITERION_ID_PATTERN.test(value);
}
