// Rubric for greenfield-novice-heavy scenario.
// Usage: bun tests/scenarios/greenfield-novice-heavy/rubric.ts <project-dir>

import {
  isChildDraftRow,
  isPlanTransitionTo,
  isTaskTransitionTo,
  loadEvidence,
  mustHave,
  runRubricMain,
  type CheckResult,
  type RubricResult,
} from "../_helpers/rubric-helpers.js";

const SCENARIO = "greenfield-novice-heavy";

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);
  const draftTaskRows = rows.filter(isChildDraftRow);

  const checks: CheckResult[] = [
    mustHave(rows, (r) => isPlanTransitionTo(r, "specified"), "plan-reached-specified", "a plan transition row with to_state=specified exists (plan from-spec ran)"),
    mustHave(rows, (r) => isPlanTransitionTo(r, "planned"), "plan-reached-planned", "a plan transition row with to_state=planned exists (plan decompose ran)"),
    mustHave(rows, (r) => isPlanTransitionTo(r, "in-progress"), "plan-reached-in-progress", "a plan transition row with to_state=in-progress exists (first claim triggered auto-advance)"),
    {
      id: "multiple-child-tasks-drafted",
      description: "at least 2 child task draft rows exist (multi-task decomposition)",
      pass: draftTaskRows.length >= 2,
      note: draftTaskRows.length < 2 ? `found ${draftTaskRows.length} child draft row(s), need at least 2` : undefined,
    },
    mustHave(rows, (r) => isTaskTransitionTo(r, "claimed"), "task-reached-claimed", "a task transition row with to_state=claimed exists"),
    mustHave(rows, (r) => isTaskTransitionTo(r, "shipped"), "task-shipped", "a task transition row with to_state=shipped exists"),
  ];

  return { scenario: SCENARIO, projectDir, pass: checks.every((c) => c.pass), checks };
}

if (import.meta.main) await runRubricMain(SCENARIO, runRubric);
