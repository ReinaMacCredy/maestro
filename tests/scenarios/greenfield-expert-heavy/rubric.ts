// Rubric for greenfield-expert-heavy scenario.
// Usage: bun tests/scenarios/greenfield-expert-heavy/rubric.ts <project-dir>

import {
  isChildDraftRow,
  isLintViolation,
  isPlanTransitionTo,
  isTaskTransitionTo,
  loadEvidence,
  mustHave,
  runRubricMain,
  type CheckResult,
  type RubricResult,
} from "../_helpers/rubric-helpers.js";

const SCENARIO = "greenfield-expert-heavy";

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);
  const childDraftRows = rows.filter(isChildDraftRow);

  const checks: CheckResult[] = [
    mustHave(rows, (r) => isPlanTransitionTo(r, "specified"), "plan-reached-specified", "a plan transition row with to_state=specified exists (plan from-spec ran)"),
    mustHave(rows, (r) => isPlanTransitionTo(r, "planned"), "plan-reached-planned", "a plan transition row with to_state=planned exists (plan decompose ran)"),
    {
      id: "multiple-child-tasks-drafted",
      description: "at least 2 child task draft rows exist (multi-task decomposition)",
      pass: childDraftRows.length >= 2,
      note: childDraftRows.length < 2 ? `found ${childDraftRows.length} child draft row(s), need at least 2` : undefined,
    },
    mustHave(rows, isLintViolation, "lint-violation-recorded", "a lint-violation row exists (intentional FAIL before recovery)"),
    mustHave(
      rows,
      (r) => isTaskTransitionTo(r, "ready") && "verdict" in r && r.verdict === "PASS",
      "task-reached-ready-pass",
      "a task transition row with to_state=ready and verdict=PASS exists (recovery verified)",
    ),
    mustHave(rows, (r) => isTaskTransitionTo(r, "shipped"), "task-shipped", "a task transition row with to_state=shipped exists"),
  ];

  return { scenario: SCENARIO, projectDir, pass: checks.every((c) => c.pass), checks };
}

if (import.meta.main) await runRubricMain(SCENARIO, runRubric);
