// Rubric for greenfield-expert-light scenario.
// Usage: bun tests/scenarios/greenfield-expert-light/rubric.ts <project-dir>

import {
  loadEvidence,
  mustHave,
  mustNotHave,
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

  const checks: CheckResult[] = [
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "task_id" in r &&
        typeof r.task_id === "string" &&
        r.task_id.length > 0 &&
        r.to_state === "draft",
      "task-reached-draft",
      "a task transition row with to_state=draft exists",
    ),
    mustHave(
      rows,
      (r): r is EvidenceRow =>
        r.kind === "transition" &&
        "task_id" in r &&
        typeof r.task_id === "string" &&
        r.task_id.length > 0 &&
        r.to_state === "claimed",
      "task-reached-claimed",
      "a task transition row with to_state=claimed exists",
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
      "a task transition row with to_state=ready and verdict=PASS exists",
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
    mustNotHave(
      rows,
      (r): r is EvidenceRow => r.kind === "lint-violation",
      "no-lint-violations",
      "no lint-violation rows (clean run expected)",
    ),
  ];

  return {
    scenario: "greenfield-expert-light",
    projectDir,
    pass: checks.every((c) => c.pass),
    checks,
  };
}

if (import.meta.main) {
  const projectDir = process.argv[2];
  if (!projectDir) {
    console.error(
      "Usage: bun tests/scenarios/greenfield-expert-light/rubric.ts <project-dir>",
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
