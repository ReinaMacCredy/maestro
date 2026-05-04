import type { EvidenceRow } from "@/features/evidence/index.js";
import { compareWitnessLevel } from "@/features/evidence/index.js";
import type { AutopilotPolicy } from "@/features/policy/index.js";
import { compareRiskClass } from "@/features/risk/index.js";
import type { Spec } from "@/features/spec/index.js";
import { scoreSpec } from "@/features/spec/index.js";
import type { Contract } from "@/features/task/index.js";
import type { Verdict } from "@/features/verdict/index.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import type { EligibilityReason, EligibilityResult } from "../domain/eligibility-types.js";

const GATING_KINDS = new Set(["command", "verifier", "ai-review", "threat-model", "plan-check"]);

export interface AutoMergeEligibleInput {
  readonly verdict: Verdict;
  readonly evidenceRows: readonly EvidenceRow[];
  readonly changedPaths: readonly string[];
  readonly sensitiveGlobs: readonly string[];
  readonly contract: Contract;
  readonly autopilotPolicy: AutopilotPolicy;
  readonly spec?: Spec;
}

export function autoMergeEligible(input: AutoMergeEligibleInput): EligibilityResult {
  const {
    verdict,
    evidenceRows,
    changedPaths,
    sensitiveGlobs,
    contract,
    autopilotPolicy,
    spec,
  } = input;

  const reasons: EligibilityReason[] = [];

  // 1. verdict-not-pass
  if (verdict.decision !== "PASS") {
    reasons.push({
      code: "verdict-not-pass",
      message: `Verdict decision is ${verdict.decision}, expected PASS.`,
    });
  }

  // 2. auto-merge-class-disabled
  const riskClass = verdict.effectiveRiskClass;
  if (autopilotPolicy.autoMergeAllowed[riskClass] === false) {
    reasons.push({
      code: "auto-merge-class-disabled",
      message: `Auto-merge is disabled for risk class "${riskClass}" by autopilot policy.`,
    });
  }

  // 3. evidence-witness-too-weak
  // Gating evidence must be witnessed-by-ci (level 2) or witnessed-by-maestro (level 1).
  // Both "witnessed-by-ci" and "witnessed-by-maestro" satisfy the threshold.
  // "agent-claimed-locally" and "agent-claimed-and-not-reproducible" do not.
  const weakEvidenceIds: string[] = [];
  for (const row of evidenceRows) {
    if (!GATING_KINDS.has(row.kind)) continue;
    // compareWitnessLevel returns -1 if row.witness_level < "witnessed-by-ci"
    if (compareWitnessLevel(row.witness_level, "witnessed-by-ci") < 0) {
      weakEvidenceIds.push(row.id);
    }
  }
  if (weakEvidenceIds.length > 0) {
    reasons.push({
      code: "evidence-witness-too-weak",
      message: `${weakEvidenceIds.length} gating evidence row(s) have witness level below "witnessed-by-ci".`,
      evidenceIds: weakEvidenceIds,
    });
  }

  // 4. forbidden-paths-touched
  const forbidden = contract.scope.filesForbidden;
  const forbiddenTouched = changedPaths.filter((p) => matchesAnyGlob(forbidden, p));
  if (forbiddenTouched.length > 0) {
    reasons.push({
      code: "forbidden-paths-touched",
      message: `${forbiddenTouched.length} changed path(s) match contract forbidden paths: ${forbiddenTouched.join(", ")}.`,
    });
  }

  // 5. sensitive-paths-untouched-without-waiver
  // Fail if changedPaths intersect sensitive globs AND no verdict-override evidence row exists.
  // "verdict-override" is not yet in the EvidenceKind union (lands at L6.5); read via string comparison.
  const sensitiveTouched = sensitiveGlobs.length > 0
    ? changedPaths.filter((p) => matchesAnyGlob(sensitiveGlobs, p))
    : [];
  if (sensitiveTouched.length > 0) {
    const hasWaiver = evidenceRows.some(
      (row) => (row.kind as string) === "verdict-override",
    );
    if (!hasWaiver) {
      reasons.push({
        code: "sensitive-paths-untouched-without-waiver",
        message: `${sensitiveTouched.length} changed path(s) match sensitive globs but no verdict-override waiver evidence exists.`,
      });
    }
  }

  // 6. rollback-not-witnessed
  const hasRollbackCiEvidence = evidenceRows.some(
    (row) =>
      row.kind === "rollback-exercised" && row.witness_level === "witnessed-by-ci",
  );
  if (!hasRollbackCiEvidence) {
    reasons.push({
      code: "rollback-not-witnessed",
      message: 'No "rollback-exercised" evidence row at "witnessed-by-ci" level found.',
    });
  }

  // 7. review-ack-missing
  // For HUMAN verdicts at risk >= medium, require a review-ack evidence row.
  if (
    verdict.decision === "HUMAN" &&
    compareRiskClass(verdict.effectiveRiskClass, "medium") >= 0
  ) {
    const hasReviewAck = evidenceRows.some(
      (row) =>
        row.kind === "review-ack" &&
        compareWitnessLevel(row.witness_level, "agent-claimed-locally") >= 0,
    );
    if (!hasReviewAck) {
      reasons.push({
        code: "review-ack-missing",
        message:
          'Verdict is HUMAN at risk >= medium but no "review-ack" evidence row found at "agent-claimed-locally" or stronger.',
      });
    }
  }

  // 8. spec-score-below-threshold
  // Pass if no spec is provided.
  if (spec !== undefined) {
    const { score } = scoreSpec(spec);
    if (score < 1.0) {
      reasons.push({
        code: "spec-score-below-threshold",
        message: `Spec quality score is ${score.toFixed(2)}, required 1.00.`,
      });
    }
  }

  return {
    eligible: reasons.length === 0,
    reasons,
  };
}
