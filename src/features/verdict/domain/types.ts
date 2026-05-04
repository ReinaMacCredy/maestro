import type { RiskClass } from "@/features/task/index.js";

export type VerdictDecision = "PASS" | "FAIL" | "HUMAN" | "BLOCK";

export type VerdictCategory =
  | "trust"
  | "evidence"
  | "policy"
  | "risk"
  | "amendment"
  | "cost-budget";

export interface VerdictReason {
  readonly category: VerdictCategory;
  readonly code: string;
  readonly message: string;
  readonly evidenceIds?: readonly string[];
  readonly findingChecks?: readonly string[];
  readonly policyRuleIds?: readonly string[];
}

export interface Verdict {
  readonly schemaVersion: 1;
  readonly id: string;
  readonly taskId: string;
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
