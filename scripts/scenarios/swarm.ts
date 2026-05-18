/**
 * Sandbox preparation + dispatch instruction printer for the scenario swarm.
 *
 * Usage:
 *   bun scripts/scenarios/swarm.ts --all
 *   bun scripts/scenarios/swarm.ts --scenarios s1,s2,...
 *
 * For each requested scenario this script:
 *   1. Creates a fresh mktemp sandbox.
 *   2. Prepares it (git init + maestro setup).
 *   3. Fills agent-brief.md placeholders and writes to the sandbox.
 *   4. Writes .maestro/scenarios/last-run.json with the scenario->dir map.
 *   5. Prints operator dispatch instructions.
 *
 * Does NOT make any LLM call. Does NOT spawn sub-agents.
 * Does NOT delete previous sandboxes.
 *
 * Exit 0 on success, 1 on error.
 */

import { $ } from "bun";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { isKnownScenario, SCENARIO_NAMES } from "./_scenarios.js";
import type { ScenarioName } from "./_scenarios.js";

const repoRoot = join(import.meta.dir, "../..");

// ---- argument parsing -------------------------------------------------------

function parseArgs(): ScenarioName[] {
  const args = process.argv.slice(2);
  if (args.length === 0 || args.includes("--all")) {
    return [...SCENARIO_NAMES];
  }
  const scenariosFlag = args.indexOf("--scenarios");
  if (scenariosFlag === -1 || !args[scenariosFlag + 1]) {
    console.error(
      "Usage: bun scripts/scenarios/swarm.ts --all\n" +
        "       bun scripts/scenarios/swarm.ts --scenarios s1,s2,...",
    );
    process.exit(1);
  }
  const names = args[scenariosFlag + 1].split(",").map((s) => s.trim());
  const unknown = names.filter((n) => !isKnownScenario(n));
  if (unknown.length > 0) {
    console.error(
      `Unknown scenario(s): ${unknown.join(", ")}\n` +
        `Known: ${SCENARIO_NAMES.join(", ")}`,
    );
    process.exit(1);
  }
  return names as ScenarioName[];
}

const scenarios = parseArgs();

// ---- helpers ----------------------------------------------------------------

async function makeTempDir(): Promise<string> {
  const result = await $`mktemp -d -t maestro-scenario-XXXXXX`.text();
  return result.trim();
}

async function prepareGreenfield(tmpdir: string): Promise<void> {
  await $`git init -q -b main`.cwd(tmpdir).quiet();
  await $`maestro setup`
    .cwd(tmpdir)
    .env({ ...process.env, MAESTRO_NO_UPDATE_CHECK: "1" })
    .quiet();
}

async function fillBrief(
  scenarioName: string,
  tmpdir: string,
): Promise<string> {
  const briefSrc = join(
    repoRoot,
    "tests/scenarios",
    scenarioName,
    "agent-brief.md",
  );
  const raw = await readFile(briefSrc, "utf8");
  const filled = raw
    .split("<SANDBOX_PATH>")
    .join(tmpdir)
    .split("<MAESTRO_CHECKOUT>")
    .join(repoRoot);
  const destDir = join(tmpdir, ".maestro/scenarios");
  await mkdir(destDir, { recursive: true });
  const destPath = join(destDir, "filled-brief.md");
  await writeFile(destPath, filled, "utf8");
  return destPath;
}

// ---- main -------------------------------------------------------------------

interface ScenarioRecord {
  name: string;
  project_dir: string;
  prepared_at: string;
  brief_path: string;
}

const preparedAt = new Date().toISOString();
const runId = `${Date.now()}-${crypto.randomUUID().slice(0, 8)}`;

console.log(`Preparing ${scenarios.length} scenario sandbox(es) in parallel...\n`);

const records: ScenarioRecord[] = await Promise.all(
  scenarios.map(async (name) => {
    const tmpdir = await makeTempDir();
    await prepareGreenfield(tmpdir);

    const briefPath = await fillBrief(name, tmpdir);

    console.log(`  ${name} -> ${tmpdir} ... done`);

    return {
      name,
      project_dir: tmpdir,
      prepared_at: new Date().toISOString(),
      brief_path: briefPath,
    };
  }),
);

// Write last-run.json into the maestro checkout's .maestro/scenarios/.
const lastRunDir = join(repoRoot, ".maestro/scenarios");
await mkdir(lastRunDir, { recursive: true });

const lastRun = {
  run_id: runId,
  prepared_at: preparedAt,
  maestro_checkout: repoRoot,
  scenarios: records,
};

const lastRunPath = join(lastRunDir, "last-run.json");
await writeFile(lastRunPath, JSON.stringify(lastRun, null, 2) + "\n", "utf8");

// ---- print dispatch instructions --------------------------------------------

const COL_NAME = 32;

console.log("\n=== SWARM SANDBOX PREP COMPLETE ===");
console.log(`Run ID: ${runId}`);
console.log(`Prepared ${records.length} scenario(s):\n`);

console.log("Scenario".padEnd(COL_NAME) + "Sandbox");
console.log("-".repeat(80));
for (const rec of records) {
  console.log(rec.name.padEnd(COL_NAME) + rec.project_dir);
}

console.log("\n=== DISPATCH INSTRUCTIONS ===\n");
console.log(
  "For each scenario above, make an Agent tool call (run_in_background: true).",
);
console.log(
  "Paste the contents of each scenario's brief_path as the agent prompt:\n",
);

for (const rec of records) {
  console.log(`  cat ${rec.brief_path}`);
}

console.log(
  "\nAfter all sub-agents complete, evaluate results:\n\n" +
    "  bun scripts/scenarios/check-all.ts",
);
