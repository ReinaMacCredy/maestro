import type { Contract, RiskClass } from "@/features/task/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import { compareWitnessLevel } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/features/verify/index.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { Verdict, VerdictReason } from "@/features/verdict/index.js";
import { generateVerdictId } from "@/features/verdict/index.js";
import { maxRiskClass } from "./risk-class-order.js";

export interface ComputeRiskInput {
  readonly contract: Contract;
  readonly trustFindings: readonly TrustFinding[];
  readonly evidenceRows: readonly EvidenceRow[];
  readonly riskPolicy: RiskPolicy;
  readonly autopilotPolicy: AutopilotPolicy;
  readonly releasePolicy: ReleasePolicy;
  readonly derivedRiskClass: RiskClass;
  readonly amendmentCount: number;
  readonly blockedAmendments?: number;
  readonly costBudgetExhausted?: boolean;
}

/**
 * Produces a Verdict from contract + trust-verifier findings + evidence +
 * policies + derived risk class.
 *
 * Decision tree (first match):
 *   1. BLOCK  — costBudgetExhausted
 *   2. FAIL   — any trust finding with severity "error"
 *   3. HUMAN  — amendmentCount > 75% of maxAmendments (Rule 5)
 *   4. HUMAN  — effectiveRiskClass === "critical" (Rule 12)
 *   5. HUMAN  — effectiveRiskClass === "high" AND any evidence below requiredWitnessLevel.high
 *   6. HUMAN  — autopilotPolicy.autoMergeAllowed[effectiveRiskClass] === false
 *   7. PASS   — otherwise
 *
 * effectiveRiskClass = max(contract.riskClass ?? "medium", derivedRiskClass).
 * Per Rule 1, the LLM-proposed value can only RAISE, never lower.
 */
export function computeRisk(input: ComputeRiskInput): Verdict {
  const {
    contract,
    trustFindings,
    evidenceRows,
    riskPolicy,
    autopilotPolicy,
    releasePolicy,
    derivedRiskClass,
    amendmentCount,
    costBudgetExhausted,
  } = input;

  const proposedRiskClass: RiskClass = contract.riskClass ?? "medium";
  const effectiveRiskClass: RiskClass = maxRiskClass(proposedRiskClass, derivedRiskClass);

  const errors = trustFindings.filter((f) => f.severity === "error");
  const warns = trustFindings.filter((f) => f.severity === "warn");
  const infos = trustFindings.filter((f) => f.severity === "info");

  const evidenceConsulted = evidenceRows.map((r) => r.id);
  const policiesConsulted = [
    { file: "policies/risk.yaml", version: riskPolicy.version },
    { file: "policies/autopilot.yaml", version: autopilotPolicy.version },
    { file: "policies/release.yaml", version: releasePolicy.version },
  ] as const;

  const trustVerifier = {
    findingsCount: trustFindings.length,
    errors: errors.length,
    warns: warns.length,
    infos: infos.length,
  };

  const reasons: VerdictReason[] = [];

  // 1. BLOCK on cost-budget exhaustion (Rule 11).
  if (costBudgetExhausted === true) {
    reasons.push({
      category: "cost-budget",
      code: "cost-budget-exhausted",
      message: "Cost budget exhausted; further execution blocked.",
    });
    return buildVerdict("BLOCK", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 2. FAIL on trust errors.
  if (errors.length > 0) {
    reasons.push({
      category: "trust",
      code: "trust-findings-error",
      message: `Trust verifier found ${errors.length} error(s).`,
      findingChecks: errors.map((f) => f.check),
    });
    return buildVerdict("FAIL", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 3. HUMAN if amendment usage exceeds 75% of budget (Rule 5).
  const maxAmendments = contract.amendmentBudget?.maxAmendments ?? 0;
  if (maxAmendments > 0 && amendmentCount > Math.floor(maxAmendments * 0.75)) {
    reasons.push({
      category: "amendment",
      code: "amendment-budget-high",
      message: `Amendment count (${amendmentCount}) exceeds 75% of budget (${maxAmendments}).`,
    });
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 4. HUMAN if effectiveRiskClass is critical (Rule 12).
  if (effectiveRiskClass === "critical") {
    reasons.push({
      category: "risk",
      code: "effective-risk-critical",
      message: `Effective risk class is critical (proposed: ${proposedRiskClass}, derived: ${derivedRiskClass}). Human review always required.`,
    });
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 5. HUMAN if high risk and any evidence is below the required witness level.
  if (effectiveRiskClass === "high") {
    const requiredLevel = autopilotPolicy.requiredWitnessLevel.high;
    const weakEvidence = evidenceRows.filter(
      (r) => compareWitnessLevel(r.witness_level, requiredLevel) < 0,
    );
    if (weakEvidence.length > 0) {
      reasons.push({
        category: "evidence",
        code: "evidence-witness-level-insufficient",
        message: `${weakEvidence.length} evidence row(s) are below the required witness level "${requiredLevel}" for high-risk tasks.`,
        evidenceIds: weakEvidence.map((r) => r.id),
      });
      return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
    }
  }

  // 6. HUMAN if policy disallows auto-merge for this risk class.
  if (autopilotPolicy.autoMergeAllowed[effectiveRiskClass] === false) {
    reasons.push({
      category: "policy",
      code: "auto-merge-not-allowed",
      message: `Auto-merge is not allowed for risk class "${effectiveRiskClass}" per autopilot policy.`,
    });
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 7. PASS.
  reasons.push({
    category: "policy",
    code: "all-checks-passed",
    message: "All checks passed.",
  });
  return buildVerdict("PASS", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
}

function buildVerdict(
  decision: Verdict["decision"],
  contract: Contract,
  proposedRiskClass: RiskClass,
  effectiveRiskClass: RiskClass,
  reasons: readonly VerdictReason[],
  evidenceConsulted: readonly string[],
  policiesConsulted: readonly { readonly file: string; readonly version: string }[],
  trustVerifier: Verdict["trustVerifier"],
): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: contract.taskId,
    contractVersion: contract.amendments.length + 1,
    computedAt: new Date().toISOString(),
    decision,
    proposedRiskClass,
    effectiveRiskClass,
    reasons,
    evidenceConsulted,
    policiesConsulted,
    trustVerifier,
  };
}
