// Rubric for greenfield-expert-heavy scenario.
// Usage: bun tests/scenarios/greenfield-expert-heavy/rubric.ts <project-dir>

import {
  loadEvidence,
  mustHave,
  type CheckResult,
  type EvidenceRow,
} from "../_helpers/rubric-helpers.js";

export interface RubricResult {
  readonly scenario: string;
  readonly projectDir: string;
  readonly pass: boolean;
  readonly checks: readonly CheckResult[];
}

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);

  const childDraftRows = rows.filter(
    (r) =>
      r.kind === "transition" &&
      "task_id" in r &&
      typeof r.task_id === "string" &&
      r.task_id.length > 0 &&
      "plan_id" in r &&
      typeof r.plan_id === "string" &&
      r.plan_id.length > 0 &&
      r.to_state === "draft",
  );

  const checks: CheckResult[] = [
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "plan_id" in r &&
        typeof r.plan_id === "string" &&
        r.plan_id.length > 0 &&
        !("task_id" in r && typeof r.task_id === "string" && r.task_id.length > 0) &&
        r.to_state === "specified",
      "plan-reached-specified",
      "a plan transition row with to_state=specified exists (plan from-spec ran)",
    ),
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "plan_id" in r &&
        typeof r.plan_id === "string" &&
        r.plan_id.length > 0 &&
        !("task_id" in r && typeof r.task_id === "string" && r.task_id.length > 0) &&
        r.to_state === "planned",
      "plan-reached-planned",
      "a plan transition row with to_state=planned exists (plan decompose ran)",
    ),
    {
      id: "multiple-child-tasks-drafted",
      description: "at least 2 child task draft rows exist (multi-task decomposition)",
      pass: childDraftRows.length >= 2,
      note:
        childDraftRows.length < 2
          ? `found ${childDraftRows.length} child draft row(s), need at least 2`
          : undefined,
    },
    mustHave(
      rows,
      (r): r is EvidenceRow => r.kind === "lint-violation",
      "lint-violation-recorded",
      "a lint-violation row exists (intentional FAIL before recovery)",
    ),
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "task_id" in r &&
        typeof r.task_id === "string" &&
        r.task_id.length > 0 &&
        r.to_state === "ready" &&
        "verdict" in r &&
        r.verdict === "PASS",
      "task-reached-ready-pass",
      "a task transition row with to_state=ready and verdict=PASS exists (recovery verified)",
    ),
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "task_id" in r &&
        typeof r.task_id === "string" &&
        r.task_id.length > 0 &&
        r.to_state === "shipped",
      "task-shipped",
      "a task transition row with to_state=shipped exists",
    ),
  ];

  return {
    scenario: "greenfield-expert-heavy",
    projectDir,
    pass: checks.every((c) => c.pass),
    checks,
  };
}

if (import.meta.main) {
  const projectDir = process.argv[2];
  if (!projectDir) {
    console.error(
      "Usage: bun tests/scenarios/greenfield-expert-heavy/rubric.ts <project-dir>",
    );
    process.exit(1);
  }
  const result = await runRubric(projectDir);
  for (const c of result.checks) {
    const marker = c.pass ? "[PASS]" : "[FAIL]";
    console.log(`${marker} ${c.id}: ${c.description}`);
    if (!c.pass && c.note) console.log(`       note: ${c.note}`);
    if (c.evidence) console.log(`       evidence: ${c.evidence}`);
  }
  console.log(result.pass ? "\nRUBRIC: PASS" : "\nRUBRIC: FAIL");
  process.exit(result.pass ? 0 : 1);
}
