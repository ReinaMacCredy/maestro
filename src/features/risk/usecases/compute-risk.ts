import type { Contract } from "@/v2/types/contract.js";
import type { RiskClass } from "@/v2/types/product-spec.js";
import type { CostBudgetExhaustionReason } from "@/v2/service/check-cost-budget.js";
import type { AIReviewPayload, EvidenceRow } from "@/features/evidence/index.js";
import { compareWitnessLevel } from "@/features/evidence/index.js";
import type { Spec } from "@/shared/domain/legacy-spec/index.js";
import type { TrustFinding } from "@/v2/types/trust.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { Verdict, VerdictReason } from "@/features/verdict/index.js";
import { generateVerdictId } from "@/features/verdict/index.js";
import { maxRiskClass, RISK_CLASS_ORDER } from "./risk-class-order.js";
import * as REASONS from "./verdict-reason-templates.js";

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
  /** Specific limit that was exceeded; surfaced in the BLOCK verdict reason so
   * agents see *which* limit hit (Edge Case 4). When absent, the reason falls
   * back to the generic exhausted message. */
  readonly costBudgetReason?: CostBudgetExhaustionReason;
  readonly matchedRiskPolicySignal?: string;
  /** Linked Spec (when the task is associated with a mission). Consulted for
   * release.yaml `require_proof_map_complete` enforcement. */
  readonly spec?: Spec;
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
    costBudgetReason,
    matchedRiskPolicySignal,
    spec,
  } = input;

  const proposedRiskClass: RiskClass = contract.riskClass ?? "medium";
  const effectiveRiskClass: RiskClass = applyCrossTaskConflictRiskRaise(
    applyAIReviewerRiskRaise(
      maxRiskClass(proposedRiskClass, derivedRiskClass),
      evidenceRows,
    ),
    evidenceRows,
  );

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
    reasons.push(REASONS.costBudgetExhausted(costBudgetReason));
    return buildVerdict("BLOCK", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 2. FAIL on trust errors.
  if (errors.length > 0) {
    const findingPaths = Array.from(new Set(errors.flatMap((f) => f.paths))).sort();
    reasons.push(REASONS.trustFindingsError({
      errorCount: errors.length,
      findingChecks: errors.map((f) => f.check),
      findingPaths,
    }));
    appendProofMapDiagnostic(reasons, spec, contract, evidenceRows);
    return buildVerdict("FAIL", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 3. HUMAN if amendment usage exceeds 75% of budget (Rule 5).
  const maxAmendments = contract.amendmentBudget?.maxAmendments ?? 0;
  if (maxAmendments > 0 && amendmentCount > Math.floor(maxAmendments * 0.75)) {
    reasons.push(REASONS.amendmentBudgetHigh({ amendmentCount, maxAmendments }));
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 4. HUMAN if effectiveRiskClass is critical (Rule 12).
  //    Also collect threat-model-required reason when the diff is security-path-related
  //    and no threat-model evidence is present (Edge Case 12).
  if (effectiveRiskClass === "critical") {
    if (requiresThreatModel(derivedRiskClass, matchedRiskPolicySignal) && !hasThreatModelEvidence(evidenceRows)) {
      reasons.push(REASONS.threatModelRequired());
    }
    reasons.push(REASONS.effectiveRiskCritical({ proposedRiskClass, derivedRiskClass }));
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 5. HUMAN if high risk and any criterion-linked evidence is below the
  //    required witness level. Only criterion-gating kinds with a criterion_id
  //    in their payload count — infra/audit rows (plan-check, review-ack,
  //    unlinked verifier, contract-amendment, verdict-override,
  //    cross-task-conflict, runtime-signal, deploy-readiness,
  //    rollback-exercised) are recorded at agent-claimed-locally by design and
  //    must not flip a verdict to HUMAN when every doneWhen criterion already
  //    has a passing witnessed-by-maestro row.
  if (effectiveRiskClass === "high") {
    const requiredLevel = autopilotPolicy.requiredWitnessLevel.high;
    const weakEvidence = evidenceRows.filter(
      (r) =>
        isCriterionLinkedEvidence(r) &&
        compareWitnessLevel(r.witness_level, requiredLevel) < 0,
    );
    if (weakEvidence.length > 0) {
      reasons.push(REASONS.evidenceWitnessLevelInsufficient({
        weakCount: weakEvidence.length,
        requiredLevel,
        evidenceIds: weakEvidence.map((r) => r.id),
      }));
      return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
    }
  }

  // 6. HUMAN if release policy requires a complete proof map and any
  //    acceptance criterion has no covering evidence row. The criteria source
  //    is the linked Spec when present, falling back to the contract's
  //    `doneWhen`. A criterion is "covered" when at least one criterion-linked
  //    evidence row carries its id.
  if (releasePolicy.requireProofMapComplete) {
    const uncovered = uncoveredCriteria(spec, contract, evidenceRows);
    if (uncovered.length > 0) {
      reasons.push(REASONS.proofMapIncomplete({ uncoveredIds: uncovered }));
      return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
    }
  }

  // 7. HUMAN if policy disallows auto-merge for this risk class.
  if (autopilotPolicy.autoMergeAllowed[effectiveRiskClass] === false) {
    reasons.push(REASONS.autoMergeNotAllowed(effectiveRiskClass));
    appendProofMapDiagnostic(reasons, spec, contract, evidenceRows);
    return buildVerdict("HUMAN", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
  }

  // 8. PASS.
  reasons.push(REASONS.allChecksPassed());
  return buildVerdict("PASS", contract, proposedRiskClass, effectiveRiskClass, reasons, evidenceConsulted, policiesConsulted, trustVerifier);
}

/**
 * Edge Case 3 (ProofMap holes): when a verdict is non-PASS for an unrelated
 * reason (trust errors or auto-merge disallowed), still surface which
 * acceptance criteria lack covering evidence. The reason field is the
 * agent-facing diagnostic; without it agents thrash on the visible failure
 * while shipping with silent coverage gaps.
 *
 * Idempotent: skips the push when the proof map is complete OR when a
 * proofMapIncomplete reason has already been recorded earlier in the chain.
 */
function appendProofMapDiagnostic(
  reasons: VerdictReason[],
  spec: Spec | undefined,
  contract: Contract,
  evidenceRows: readonly EvidenceRow[],
): void {
  const uncovered = uncoveredCriteria(spec, contract, evidenceRows);
  if (uncovered.length === 0) return;
  if (reasons.some((r) => r.code === "proof-map-incomplete")) return;
  reasons.push(REASONS.proofMapIncomplete({ uncoveredIds: uncovered }));
}

function uncoveredCriteria(
  spec: Spec | undefined,
  contract: Contract,
  evidenceRows: readonly EvidenceRow[],
): readonly string[] {
  const criteria = spec?.acceptance_criteria ?? contract.doneWhen ?? [];
  if (criteria.length === 0) return [];
  const coveredIds = new Set<string>();
  for (const row of evidenceRows) {
    if (!isCriterionLinkedEvidence(row)) continue;
    const id = (row.payload as { criterion_id?: string }).criterion_id;
    if (typeof id === "string" && id.length > 0) coveredIds.add(id);
  }
  return criteria
    .filter((c) => !coveredIds.has(c.id))
    .map((c) => c.id);
}

/**
 * Applies ai-review Evidence rows to potentially RAISE the effective risk class.
 * Per Rule 1 (LLM veto-only): a clean review never lowers; only error-severity
 * findings raise the class.
 *
 * - security reviewer error → always critical
 * - bug/architecture reviewer error → raise by one notch
 */
export function applyAIReviewerRiskRaise(
  effectiveRiskClass: RiskClass,
  evidenceRows: readonly EvidenceRow[],
): RiskClass {
  let current = effectiveRiskClass;
  for (const row of evidenceRows) {
    if (row.kind !== "ai-review") continue;
    const payload = row.payload as AIReviewPayload;
    const hasError = payload.findings.some((f) => f.severity === "error");
    if (!hasError) continue;
    if (payload.reviewer === "security") {
      current = "critical";
    } else {
      // Raise by one notch: low→medium, medium→high, high→critical, critical→critical
      const idx = RISK_CLASS_ORDER.indexOf(current);
      const raised = RISK_CLASS_ORDER[Math.min(idx + 1, RISK_CLASS_ORDER.length - 1)];
      if (raised !== undefined) current = raised;
    }
  }
  return current;
}

/**
 * Applies cross-task-conflict Evidence rows to potentially RAISE the effective
 * risk class by one tier. Multiple rows still raise by ONE tier total (capped at
 * critical), mirroring the ai-review raise pattern.
 *
 * Presence of any cross-task-conflict row (regardless of count) raises once:
 *   low → medium, medium → high, high → critical, critical → critical
 */
export function applyCrossTaskConflictRiskRaise(
  effectiveRiskClass: RiskClass,
  evidenceRows: readonly EvidenceRow[],
): RiskClass {
  const hasConflict = evidenceRows.some((row) => row.kind === "cross-task-conflict");
  if (!hasConflict) return effectiveRiskClass;
  const idx = RISK_CLASS_ORDER.indexOf(effectiveRiskClass);
  const raised = RISK_CLASS_ORDER[Math.min(idx + 1, RISK_CLASS_ORDER.length - 1)];
  return raised !== undefined ? raised : effectiveRiskClass;
}

/**
 * Returns true when the derived risk class is critical AND the matched policy
 * signal is security-path-related (Edge Case 12).
 *
 * The only signal that means "this diff touches security-relevant sensitive
 * paths" is "diff-intersects-sensitive-security" from derive-risk-class.ts.
 */
export function requiresThreatModel(
  derivedRiskClass: RiskClass,
  matchedSignal: string | undefined,
): boolean {
  return derivedRiskClass === "critical" && matchedSignal === "diff-intersects-sensitive-security";
}

/**
 * Returns true if any evidence row has kind "threat-model".
 * Per Rule 1, schema-valid presence is necessary but not sufficient —
 * empty-content rows still clear this predicate.
 */
export function hasThreatModelEvidence(evidenceRows: readonly EvidenceRow[]): boolean {
  return evidenceRows.some((r) => r.kind === "threat-model");
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

/**
 * True when the row could plausibly cover a contract doneWhen criterion: a
 * criterion-evidence kind whose payload carries a `criterion_id` field. Rows
 * without a criterion link are infrastructure/audit/diagnostic and must not
 * gate the witness-level check (Rule: ProofMap is what proves coverage; this
 * predicate is the inverse — "can this row participate in coverage at all?").
 */
function isCriterionLinkedEvidence(row: EvidenceRow): boolean {
  if (row.kind !== "command" && row.kind !== "manual-note" && row.kind !== "ai-review" && row.kind !== "threat-model") {
    return false;
  }
  const payload = row.payload as { criterion_id?: unknown };
  return typeof payload.criterion_id === "string" && payload.criterion_id.length > 0;
}
