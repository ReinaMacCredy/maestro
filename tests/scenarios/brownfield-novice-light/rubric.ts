// Rubric for brownfield-novice-light scenario.
// Usage: bun tests/scenarios/brownfield-novice-light/rubric.ts <project-dir>

import { join } from "node:path";
import {
  isTaskTransitionTo,
  loadEvidence,
  loadMigrationFlag,
  mustExistFile,
  mustHave,
  runRubricMain,
  type CheckResult,
  type RubricResult,
} from "../_helpers/rubric-helpers.js";

const SCENARIO = "brownfield-novice-light";

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);
  const migrationFlag = await loadMigrationFlag(projectDir);

  const checks: CheckResult[] = [
    {
      id: "migration-flag-present",
      description: ".maestro/.migrated-v2.json is present (setup migrate-v2 ran)",
      pass: migrationFlag !== null,
      note: migrationFlag === null ? ".maestro/.migrated-v2.json not found" : undefined,
    },
    await mustExistFile(
      join(projectDir, "docs/principles/legacy/legacy-rule-1.md"),
      "legacy-principle-migrated",
      "docs/principles/legacy/legacy-rule-1.md exists (corrections migrated)",
    ),
    mustHave(rows, (r) => isTaskTransitionTo(r, "draft"), "task-reached-draft", "a task transition row with to_state=draft exists"),
    mustHave(rows, (r) => isTaskTransitionTo(r, "claimed"), "task-reached-claimed", "a task transition row with to_state=claimed exists"),
    mustHave(
      rows,
      (r) => isTaskTransitionTo(r, "ready") && "verdict" in r && r.verdict === "PASS",
      "task-reached-ready-pass",
      "a task transition row with to_state=ready and verdict=PASS exists",
    ),
    mustHave(rows, (r) => isTaskTransitionTo(r, "shipped"), "task-shipped", "a task transition row with to_state=shipped exists"),
  ];

  return { scenario: SCENARIO, projectDir, pass: checks.every((c) => c.pass), checks };
}

if (import.meta.main) await runRubricMain(SCENARIO, runRubric);
