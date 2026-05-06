import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { compareRiskClass } from "@/features/risk/index.js";
import type { RiskClass } from "@/features/task/index.js";
import type { Contract } from "@/features/task/index.js";
import type { Spec } from "@/features/spec/index.js";
import type { PlanCheckFinding, PlanCheckResult, PlanInput } from "../domain/types.js";

export interface CheckPlanInput {
  readonly plan: PlanInput;
  readonly contract: Contract;
  readonly spec?: Spec;
  readonly derivedRiskClass: RiskClass;
}

export function checkPlan(input: CheckPlanInput): PlanCheckResult {
  const { plan, contract, spec, derivedRiskClass } = input;
  const findings: PlanCheckFinding[] = [];

  // Check 1 — scope-widens
  const filesExpected = contract.scope.filesExpected;
  if (filesExpected.length > 0) {
    const outOfScope = plan.intendedFiles.filter(
      (f) => !matchesAnyGlob(filesExpected, f),
    );
    if (outOfScope.length > 0) {
      findings.push({
        check: "scope-widens",
        severity: "error",
        message: `${outOfScope.length} intended file(s) are outside the contract scope.`,
        paths: outOfScope,
      });
    }
  }

  // Check 2 — missing-proof
  const allCriteria = [
    ...(spec?.acceptance_criteria ?? []),
    ...contract.doneWhen,
  ];
  if (allCriteria.length > 0) {
    const coveredIds = new Set(plan.proofSet.map((p) => p.criterionId));
    const missingIds = allCriteria
      .map((c) => c.id)
      .filter((id) => !coveredIds.has(id));
    if (missingIds.length > 0) {
      findings.push({
        check: "missing-proof",
        severity: "error",
        message: `${missingIds.length} criterion/criteria not covered by the proof set.`,
        criterionIds: missingIds,
      });
    }
  }

  // Check 3 — risk-class-too-low
  if (compareRiskClass(plan.riskClass, derivedRiskClass) < 0) {
    findings.push({
      check: "risk-class-too-low",
      severity: "error",
      message: `Plan proposes risk class "${plan.riskClass}" but derived class is "${derivedRiskClass}". Agent cannot lower the derived class (Rule 1).`,
    });
  }

  const errorCount = findings.filter((f) => f.severity === "error").length;
  const warnCount = findings.filter((f) => f.severity === "warn").length;

  return { findings, errorCount, warnCount };
}
