import type { EvidenceKind } from "@/features/evidence/index.js";
import type { RiskClass } from "@/types/product-spec.js";

export interface PlanInput {
  readonly intendedFiles: readonly string[];
  readonly proofSet: readonly {
    readonly criterionId: string;
    readonly evidenceKinds: readonly EvidenceKind[];
  }[];
  readonly riskClass: RiskClass;
  readonly notes?: string;
}

export interface PlanCheckFinding {
  readonly check: "scope-widens" | "missing-proof" | "risk-class-too-low";
  readonly severity: "info" | "warn" | "error";
  readonly message: string;
  readonly paths?: readonly string[];
  readonly criterionIds?: readonly string[];
}

export interface PlanCheckResult {
  readonly findings: readonly PlanCheckFinding[];
  readonly errorCount: number;
  readonly warnCount: number;
}
