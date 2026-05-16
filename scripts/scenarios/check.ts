/**
 * Single-scenario rubric runner.
 *
 * Usage: bun scripts/scenarios/check.ts <scenario-name> <project-dir>
 *
 * Exit 0 = PASS, exit 1 = FAIL, exit 2 = usage/validation error.
 */

import { stat } from "node:fs/promises";
import { join } from "node:path";
import { isKnownScenario, SCENARIO_NAMES } from "./_scenarios.js";
import {
  printRubricResult,
  type RubricResult,
} from "../../tests/scenarios/_helpers/rubric-helpers.js";

const repoRoot = join(import.meta.dir, "../..");

function die(msg: string, code = 2): never {
  console.error(msg);
  process.exit(code);
}

const [, , scenarioName, projectDir] = process.argv;

if (!scenarioName || !projectDir) {
  die(`Usage: bun scripts/scenarios/check.ts <scenario-name> <project-dir>
Known scenarios:\n  ${SCENARIO_NAMES.join("\n  ")}`);
}

if (!isKnownScenario(scenarioName)) {
  die(
    `Unknown scenario: ${scenarioName}\nKnown scenarios:\n  ${SCENARIO_NAMES.join("\n  ")}`,
  );
}

let dirOk = false;
try {
  const s = await stat(projectDir);
  dirOk = s.isDirectory();
} catch {
  /* handled below */
}
if (!dirOk) {
  die(`project-dir does not exist or is not a directory: ${projectDir}`);
}

// Dynamically import the rubric for this scenario.
const rubricPath = join(repoRoot, "tests/scenarios", scenarioName, "rubric.ts");
const mod = await import(rubricPath) as { runRubric(dir: string): Promise<RubricResult> };
const result = await mod.runRubric(projectDir);

printRubricResult(scenarioName, result);
process.exit(result.pass ? 0 : 1);
