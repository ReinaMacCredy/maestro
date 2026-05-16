// Rubric for greenfield-novice-light scenario.
// Usage: bun tests/scenarios/greenfield-novice-light/rubric.ts <project-dir>

import {
  isLintViolation,
  isTaskTransitionTo,
  loadEvidence,
  mustHave,
  mustNotHave,
  runRubricMain,
  type CheckResult,
  type RubricResult,
} from "../_helpers/rubric-helpers.js";

const SCENARIO = "greenfield-novice-light";

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);

  const checks: CheckResult[] = [
    mustHave(rows, (r) => isTaskTransitionTo(r, "draft"), "task-reached-draft", "a task transition row with to_state=draft exists"),
    mustHave(rows, (r) => isTaskTransitionTo(r, "claimed"), "task-reached-claimed", "a task transition row with to_state=claimed exists"),
    mustHave(
      rows,
      (r) => isTaskTransitionTo(r, "ready") && "verdict" in r && r.verdict === "PASS",
      "task-reached-ready-pass",
      "a task transition row with to_state=ready and verdict=PASS exists",
    ),
    mustHave(rows, (r) => isTaskTransitionTo(r, "shipped"), "task-shipped", "a task transition row with to_state=shipped exists"),
    mustNotHave(rows, isLintViolation, "no-lint-violations", "no lint-violation rows (clean run expected)"),
  ];

  return { scenario: SCENARIO, projectDir, pass: checks.every((c) => c.pass), checks };
}

if (import.meta.main) await runRubricMain(SCENARIO, runRubric);
