import type { Verdict } from "@/features/verdict/index.js";
import type { RiskPolicy } from "@/features/policy/index.js";
import type { DerivedRiskInput, DerivedRiskResult } from "./domain/types.js";
import type { ComputeRiskInput } from "./usecases/compute-risk.js";
import { computeRisk } from "./usecases/compute-risk.js";
import { deriveRiskClassFromDiff } from "./usecases/derive-risk-class.js";

export interface RiskServices {
  computeRisk: (input: ComputeRiskInput) => Verdict;
  deriveRiskClassFromDiff: (input: DerivedRiskInput, policy?: RiskPolicy) => DerivedRiskResult;
}

export function buildRiskServices(): RiskServices {
  return {
    computeRisk,
    deriveRiskClassFromDiff,
  };
}
