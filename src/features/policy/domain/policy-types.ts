import type { RiskClass } from "@/v2/types/product-spec.js";
import type { WitnessLevel } from "@/features/evidence/index.js";

export type PolicyKind = "risk" | "autopilot" | "release" | "sensitive-paths" | "owners";

export interface PolicyRule {
  readonly id: string;
  readonly description?: string;
}

export interface RiskPolicyRow {
  readonly signal: string;
  readonly derivedClass: RiskClass;
  readonly description?: string;
}

export interface RiskPolicy extends PolicyRule {
  readonly kind: "risk";
  readonly rows: readonly RiskPolicyRow[];
  readonly version: string;
}

export interface AutopilotPolicy extends PolicyRule {
  readonly kind: "autopilot";
  readonly autoMergeAllowed: {
    readonly low: boolean;
    readonly medium: boolean;
    readonly high: boolean;
    readonly critical: boolean;
  };
  readonly requiredWitnessLevel: {
    readonly low: WitnessLevel;
    readonly medium: WitnessLevel;
    readonly high: WitnessLevel;
    readonly critical: WitnessLevel;
  };
  readonly version: string;
}

export interface ReleasePolicy extends PolicyRule {
  readonly kind: "release";
  readonly requireSignedCommits: boolean;
  readonly requireProofMapComplete: boolean;
  readonly version: string;
}
