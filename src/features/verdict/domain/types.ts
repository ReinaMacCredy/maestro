import type { RiskClass } from "@/features/task/index.js";

export interface VerdictSubject {
  readonly pr?: number;
  readonly tree_sha: string;
}

export type VerdictDecision = "PASS" | "FAIL" | "HUMAN" | "BLOCK";

export type VerdictCategory =
  | "trust"
  | "evidence"
  | "policy"
  | "risk"
  | "amendment"
  | "cost-budget";

export type VerdictReasonCode =
  | "cost-budget-exhausted"
  | "trust-findings-error"
  | "amendment-budget-high"
  | "effective-risk-critical"
  | "evidence-witness-level-insufficient"
  | "proof-map-incomplete"
  | "auto-merge-not-allowed"
  | "all-checks-passed"
  | "threat-model-required";

export interface VerdictReason {
  readonly category: VerdictCategory;
  readonly code: VerdictReasonCode;
  readonly message: string;
  readonly evidenceIds?: readonly string[];
  readonly findingChecks?: readonly string[];
  readonly findingPaths?: readonly string[];
  readonly policyRuleIds?: readonly string[];
}

export interface Verdict {
  readonly schemaVersion: 1;
  readonly id: string;
  readonly taskId: string;
  readonly subject?: VerdictSubject;
  readonly contractVersion: number;
  readonly computedAt: string;
  readonly decision: VerdictDecision;
  readonly proposedRiskClass?: RiskClass;
  readonly effectiveRiskClass: RiskClass;
  readonly reasons: readonly VerdictReason[];
  readonly evidenceConsulted: readonly string[];
  readonly policiesConsulted: readonly { readonly file: string; readonly version: string }[];
  readonly trustVerifier: {
    readonly findingsCount: number;
    readonly errors: number;
    readonly warns: number;
    readonly infos: number;
  };
}
