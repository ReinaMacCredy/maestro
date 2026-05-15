import type { MilestoneProfile } from "@/shared/domain/legacy-mission";

export type PrincipleMode = "advisory" | "gate";
export type GateCheckType = `array_min_length:${number}` | "object_non_empty" | "array_all_passed";
export type PrincipleSource = "karpathy" | "custom";

export interface Principle {
  readonly id: string;
  readonly name: string;
  readonly source: PrincipleSource;
  readonly rule: string;
  readonly profiles: readonly MilestoneProfile[];
  readonly mode: PrincipleMode;
  readonly gateField?: string;
  readonly gateCheck?: GateCheckType;
}

export interface CreatePrincipleInput {
  readonly id: string;
  readonly name: string;
  readonly source?: PrincipleSource;
  readonly rule: string;
  readonly profiles: readonly string[];
  readonly mode: PrincipleMode;
  readonly gateField?: string;
  readonly gateCheck?: string;
}

/** Outcome attribution for a principle gate against a single handoff. */
export type PrincipleOutcome = "pending" | "helpful" | "unhelpful";

/**
 * One row in `.maestro/principles/outcomes.jsonl`. Append-only:
 * later rows for the same (principleId, handoffId) pair supersede earlier
 * ones; the effectiveness aggregator takes the last state.
 */
export interface PrincipleOutcomeRecord {
  readonly principleId: string;
  readonly handoffId: string;
  readonly featureId?: string;
  readonly missionId?: string;
  readonly outcome: PrincipleOutcome;
  readonly recordedAt: string;
}

/** Rolled-up effectiveness stats for a single principle. */
export interface PrincipleEffectiveness {
  readonly principleId: string;
  readonly helpful: number;
  readonly unhelpful: number;
  readonly pending: number;
  readonly total: number;
  /** helpful / (helpful + unhelpful) expressed as 0..1, or undefined if both are zero. */
  readonly effectiveness?: number;
}
