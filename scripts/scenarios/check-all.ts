/**
 * Multi-scenario rubric runner.
 *
 * Reads .maestro/scenarios/last-run.json from the maestro checkout root,
 * then evaluates each recorded scenario's rubric against its sandbox dir.
 *
 * Usage: bun scripts/scenarios/check-all.ts
 *
 * Exit 0 = all PASS, exit 1 = any FAIL, exit 2 = no run recorded.
 */

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import type { RubricResult } from "../../tests/scenarios/greenfield-novice-light/rubric.js";

const repoRoot = join(import.meta.dir, "../..");
const lastRunPath = join(repoRoot, ".maestro/scenarios/last-run.json");

interface ScenarioRecord {
  name: string;
  project_dir: string;
  project_type: "greenfield" | "brownfield";
  prepared_at: string;
  brief_path: string;
}

interface LastRun {
  run_id: string;
  prepared_at: string;
  maestro_checkout: string;
  scenarios: ScenarioRecord[];
}

let lastRun: LastRun;
try {
  const raw = await readFile(lastRunPath, "utf8");
  lastRun = JSON.parse(raw) as LastRun;
} catch {
  console.error("no swarm run recorded — run swarm.ts first");
  process.exit(2);
}

if (!Array.isArray(lastRun.scenarios) || lastRun.scenarios.length === 0) {
  console.error("last-run.json has no scenarios — run swarm.ts first");
  process.exit(2);
}

// ---- run rubrics ------------------------------------------------------------

interface ScenarioOutcome {
  name: string;
  result: RubricResult;
}

const outcomes: ScenarioOutcome[] = [];

for (const rec of lastRun.scenarios) {
  const rubricPath = join(repoRoot, "tests/scenarios", rec.name, "rubric.ts");
  const mod = await import(rubricPath) as { runRubric(dir: string): Promise<RubricResult> };
  const result = await mod.runRubric(rec.project_dir);
  outcomes.push({ name: rec.name, result });
}

// ---- summary table ----------------------------------------------------------

const COL_SCENARIO = 28;
const COL_STATUS = 8;

const header =
  "SCENARIO".padEnd(COL_SCENARIO) + "STATUS".padEnd(COL_STATUS) + "CHECKS";
console.log(header);
console.log("-".repeat(header.length + 10));

let totalPass = 0;
const failures: { name: string; failedChecks: string[] } [] = [];

for (const { name, result } of outcomes) {
  const passCount = result.checks.filter((c) => c.pass).length;
  const total = result.checks.length;
  const status = result.pass ? "PASS" : "FAIL";
  if (result.pass) {
    totalPass++;
  } else {
    failures.push({
      name,
      failedChecks: result.checks.filter((c) => !c.pass).map((c) => c.id),
    });
  }
  const failNote =
    !result.pass && failures[failures.length - 1]
      ? `    [${failures[failures.length - 1].failedChecks.join(", ")} FAIL]`
      : "";
  console.log(
    `${name.padEnd(COL_SCENARIO)}${status.padEnd(COL_STATUS)}${passCount}/${total}${failNote}`,
  );
}

console.log();
console.log(`OVERALL: ${totalPass}/${outcomes.length} PASS`);

process.exit(totalPass === outcomes.length ? 0 : 1);
