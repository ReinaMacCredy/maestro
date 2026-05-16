// Rubric for brownfield-novice-light scenario.
// Usage: bun tests/scenarios/brownfield-novice-light/rubric.ts <project-dir>

import { join } from "node:path";
import {
  loadEvidence,
  loadMigrationFlag,
  mustHave,
  mustExistFile,
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
  const migrationFlag = await loadMigrationFlag(projectDir);

  const migrationCheck: CheckResult = {
    id: "migration-flag-present",
    description: ".maestro/.migrated-v2.json is present (setup migrate-v2 ran)",
    pass: migrationFlag !== null,
    note: migrationFlag === null ? ".maestro/.migrated-v2.json not found" : undefined,
  };

  const legacyPrincipleCheck = await mustExistFile(
    join(projectDir, "docs/principles/legacy/legacy-rule-1.md"),
    "legacy-principle-migrated",
    "docs/principles/legacy/legacy-rule-1.md exists (corrections migrated)",
  );

  const checks: CheckResult[] = [
    migrationCheck,
    legacyPrincipleCheck,
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
  ];

  return {
    scenario: "brownfield-novice-light",
    projectDir,
    pass: checks.every((c) => c.pass),
    checks,
  };
}

if (import.meta.main) {
  const projectDir = process.argv[2];
  if (!projectDir) {
    console.error(
      "Usage: bun tests/scenarios/brownfield-novice-light/rubric.ts <project-dir>",
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
