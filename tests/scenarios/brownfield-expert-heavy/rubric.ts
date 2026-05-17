// Rubric for brownfield-expert-heavy scenario.
// Usage: bun tests/scenarios/brownfield-expert-heavy/rubric.ts <project-dir>

import { join } from "node:path";
import {
  isChildDraftRow,
  isMissionTransitionTo,
  isTaskTransitionTo,
  loadEvidence,
  loadMigrationFlag,
  mustExistFile,
  mustHave,
  runRubricMain,
  type CheckResult,
  type RubricResult,
} from "../_helpers/rubric-helpers.js";

const SCENARIO = "brownfield-expert-heavy";

export async function runRubric(projectDir: string): Promise<RubricResult> {
  const rows = await loadEvidence(projectDir);
  const migrationFlag = await loadMigrationFlag(projectDir);
  const childDraftRows = rows.filter(isChildDraftRow);

  const checks: CheckResult[] = [
    {
      id: "migration-flag-present",
      description: ".maestro/.migrated-v2.json is present (setup migration ran)",
      pass: migrationFlag !== null,
      note: migrationFlag === null ? ".maestro/.migrated-v2.json not found" : undefined,
    },
    await mustExistFile(
      join(projectDir, "docs/principles/legacy/legacy-rule-1.md"),
      "legacy-principle-migrated",
      "docs/principles/legacy/legacy-rule-1.md exists (corrections migrated)",
    ),
    mustHave(rows, (r) => isMissionTransitionTo(r, "approved"), "mission-reached-approved", "a mission transition row with to_state=approved exists"),
    mustHave(rows, (r) => isMissionTransitionTo(r, "planned"), "mission-reached-planned", "a mission transition row with to_state=planned exists"),
    {
      id: "multiple-child-tasks-drafted",
      description: "at least 2 child task draft rows exist",
      pass: childDraftRows.length >= 2,
      note: childDraftRows.length < 2 ? `found ${childDraftRows.length} child draft row(s), need at least 2` : undefined,
    },
    mustHave(
      rows,
      (r) => isTaskTransitionTo(r, "blocked") && "verdict" in r && r.verdict === "BLOCK",
      "task-blocked-with-verdict",
      "a task transition row with to_state=blocked and verdict=BLOCK exists (explicit block)",
    ),
    mustHave(
      rows,
      (r) => isTaskTransitionTo(r, "ready") && "verdict" in r && r.verdict === "PASS",
      "task-reached-ready-pass",
      "a task transition row with to_state=ready and verdict=PASS exists (recovery)",
    ),
    mustHave(rows, (r) => isTaskTransitionTo(r, "shipped"), "task-shipped", "a task transition row with to_state=shipped exists"),
  ];

  return { scenario: SCENARIO, projectDir, pass: checks.every((c) => c.pass), checks };
}

if (import.meta.main) await runRubricMain(SCENARIO, runRubric);
