/**
 * UKI handoff domain types.
 *
 * Kept separate from `src/domain/types.ts` so the new Phase 2 handoff
 * shape does not pollute the slimmer post-Phase-1 core types. The old
 * Handoff / HandoffEnvelope / HandoffSession types were deleted in
 * Phase 1 and are replaced by UkiHandoff + UkiSlots below.
 */
import type { UkiSlots } from "../lib/uki-format.js";

export const UKI_HANDOFF_STATUSES = ["pending", "picked-up", "completed"] as const;

export type UkiHandoffStatus = (typeof UKI_HANDOFF_STATUSES)[number];

export const UKI_HANDOFF_VERSION = "5.3";
export const SUPPORTED_UKI_HANDOFF_VERSIONS = ["5.2", UKI_HANDOFF_VERSION] as const;

export type UkiHandoffVersion = (typeof SUPPORTED_UKI_HANDOFF_VERSIONS)[number];

/**
 * A persisted UKI handoff record.
 *
 * `slots` is the source of truth -- the structured data the creator
 * supplied. `uki` is the cached compressed string generated from `slots`
 * at creation time, so external workers can `pickup --uki` without the
 * store re-compressing on every read.
 */
export interface UkiHandoff {
  readonly id: string;
  readonly version: UkiHandoffVersion;
  readonly timestamp: string;
  readonly status: UkiHandoffStatus;
  readonly agent: string;
  readonly sessionId: string;
  readonly slots: UkiSlots;
  readonly uki: string;
  readonly pickedUpAt?: string;
  readonly pickedUpBy?: string;
  readonly completedAt?: string;
  readonly report?: string;
}

export interface CreateUkiHandoffInput {
  readonly slots: UkiSlots;
  readonly agent: string;
  readonly sessionId: string;
}
