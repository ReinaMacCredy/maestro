import type { RiskClass } from "@/types/product-spec.js";
import type { CostBudgetExhaustionReason } from "@/service/check-cost-budget.js";
import type { VerdictReason } from "@/features/verdict/index.js";

/**
 * Verdict reason templates extracted from compute-risk.ts. Each factory
 * returns a {@link VerdictReason} with the same shape and content the prior
 * inline literals produced. Pure functions, no I/O. Behavior-preserving:
 * existing tests assert only on `reason.code`, never on `reason.message`.
 */

const COST_BUDGET_LIMIT_LABEL: Record<CostBudgetExhaustionReason, string> = {
  "max-retries": "maxRetries",
  "max-wall-clock": "maxWallClockSeconds",
  "max-tokens": "maxTokens",
};

export function costBudgetExhausted(reason?: CostBudgetExhaustionReason): VerdictReason {
  const limitClause = reason !== undefined
    ? `Exhausted limit: \`costBudget.${COST_BUDGET_LIMIT_LABEL[reason]}\` (reason=${reason}). `
    : "";
  return {
    category: "cost-budget",
    code: "cost-budget-exhausted",
    message:
      `Cost budget exhausted; further execution blocked. ${limitClause}` +
      "Inspect the current budget on `maestro contract show --task <id>`, " +
      "amend the contract's costBudget via `maestro contract amend` " +
      "to raise the cap, or block the task with a reason for human pickup " +
      "via `maestro task block <id> --reason \"...\"`.",
    ...(reason !== undefined ? { findingChecks: [reason] } : {}),
  };
}

export function trustFindingsError(args: {
  errorCount: number;
  findingChecks: readonly string[];
  findingPaths: readonly string[];
}): VerdictReason {
  const { errorCount, findingChecks, findingPaths } = args;
  return {
    category: "trust",
    code: "trust-findings-error",
    message: `Trust verifier found ${errorCount} error(s).`,
    findingChecks,
    ...(findingPaths.length > 0 ? { findingPaths } : {}),
  };
}

export function amendmentBudgetHigh(args: {
  amendmentCount: number;
  maxAmendments: number;
}): VerdictReason {
  return {
    category: "amendment",
    code: "amendment-budget-high",
    message: `Amendment count (${args.amendmentCount}) exceeds 75% of budget (${args.maxAmendments}).`,
  };
}

export function threatModelRequired(): VerdictReason {
  return {
    category: "policy",
    code: "threat-model-required",
    message:
      "Diff intersects security-relevant sensitive paths with critical risk class; a threat-model Evidence row is required.",
  };
}

export function effectiveRiskCritical(args: {
  proposedRiskClass: RiskClass;
  derivedRiskClass: RiskClass;
}): VerdictReason {
  return {
    category: "risk",
    code: "effective-risk-critical",
    message: `Effective risk class is critical (proposed: ${args.proposedRiskClass}, derived: ${args.derivedRiskClass}). Human review always required.`,
  };
}

export function evidenceWitnessLevelInsufficient(args: {
  weakCount: number;
  requiredLevel: string;
  evidenceIds: readonly string[];
}): VerdictReason {
  return {
    category: "evidence",
    code: "evidence-witness-level-insufficient",
    message: `${args.weakCount} evidence row(s) are below the required witness level "${args.requiredLevel}" for high-risk tasks.`,
    evidenceIds: args.evidenceIds,
  };
}

export function proofMapIncomplete(args: {
  uncoveredIds: readonly string[];
  uncoveredCriteria?: readonly { readonly id: string; readonly text: string }[];
}): VerdictReason {
  const labels = (args.uncoveredCriteria ?? []).map((c) => `${c.id} ("${c.text}")`);
  const list = labels.length > 0 ? labels.join("; ") : args.uncoveredIds.join(", ");
  const example = args.uncoveredIds[0] ?? "<criterion_id>";
  return {
    category: "evidence",
    code: "proof-map-incomplete",
    message:
      `${args.uncoveredIds.length} acceptance criterion/criteria lack covering evidence: ${list}. ` +
      `Record covering evidence with \`maestro evidence record --task <id> --criterion ${example} --command "..." --exit 0\` ` +
      `(criterion id matches the spec's acceptance_criteria — see \`maestro spec show <slug>\` or the spec frontmatter).`,
  };
}

export function autoMergeNotAllowed(effectiveRiskClass: RiskClass): VerdictReason {
  return {
    category: "policy",
    code: "auto-merge-not-allowed",
    message: `Auto-merge is opt-in and not enabled for risk class "${effectiveRiskClass}" in autopilot.yaml; the task can still complete via human review (run \`maestro handoff emit\`, or set autoMergeAllowed.${effectiveRiskClass}: true — note: enabling auto-merge is a "loosening" and soaks for 30 days before taking effect; run \`maestro policy pending\` to see in-flight changes).`,
  };
}

export function allChecksPassed(): VerdictReason {
  return {
    category: "policy",
    code: "all-checks-passed",
    message: "All checks passed.",
  };
}
