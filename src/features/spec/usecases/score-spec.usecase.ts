import type { Spec } from "../domain/types.js";

export interface SpecScoreResult {
  readonly score: number;
  readonly populatedSlots: readonly string[];
  readonly missingSlots: readonly string[];
}

const SLOTS = ["acceptance_criteria", "non_goals"] as const;

export function scoreSpec(spec: Spec): SpecScoreResult {
  const populatedSlots: string[] = [];
  const missingSlots: string[] = [];

  if (spec.acceptance_criteria.length >= 1) {
    populatedSlots.push("acceptance_criteria");
  } else {
    missingSlots.push("acceptance_criteria");
  }

  if (spec.non_goals.length >= 1) {
    populatedSlots.push("non_goals");
  } else {
    missingSlots.push("non_goals");
  }

  const score = populatedSlots.length / SLOTS.length;
  return { score, populatedSlots, missingSlots };
}
