import type { Verdict } from "@/features/verdict/index.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { DerivedRiskInput, DerivedRiskResult } from "./domain/types.js";
import type { ComputeRiskInput } from "./usecases/compute-risk.js";
import { computeRisk } from "./usecases/compute-risk.js";
import { deriveRiskClassFromDiff } from "./usecases/derive-risk-class.js";

export interface RiskPolicyGetters {
  readonly getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  readonly getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  readonly getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
}

export interface RiskServices {
  computeRisk: (input: ComputeRiskInput) => Verdict;
  deriveRiskClassFromDiff: (input: DerivedRiskInput, policy?: RiskPolicy) => DerivedRiskResult;
  /** Resolve the effective policies for use when building a ComputeRiskInput */
  getEffectivePolicies: () => Promise<{
    riskPolicy: RiskPolicy;
    autopilotPolicy: AutopilotPolicy;
    releasePolicy: ReleasePolicy;
  }>;
}

export function buildRiskServices(policyGetters?: RiskPolicyGetters): RiskServices {
  return {
    computeRisk,
    deriveRiskClassFromDiff,
    async getEffectivePolicies() {
      if (!policyGetters) {
        throw new Error(
          "getEffectivePolicies requires policy getters — pass them via buildRiskServices(policyGetters)",
        );
      }
      const [riskPolicy, autopilotPolicy, releasePolicy] = await Promise.all([
        policyGetters.getEffectiveRiskPolicy(),
        policyGetters.getEffectiveAutopilotPolicy(),
        policyGetters.getEffectiveReleasePolicy(),
      ]);
      return { riskPolicy, autopilotPolicy, releasePolicy };
    },
  };
}
