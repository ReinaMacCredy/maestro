/**
 * v2 Hot-Path Smoke Test
 *
 * Exercises the full v2 task lifecycle against a freshly installed maestro
 * binary. Intended for use in CI after install and before release publish.
 *
 * Usage:
 *   bun scripts/v2-smoke.ts                     # uses maestro on PATH
 *   bun scripts/v2-smoke.ts ./dist/maestro       # uses a specific binary
 *
 * Exits non-zero on any v2 verb failure.
 *
 * Note: `spec new --from-file` is not a real CLI verb (as of v0.83.0).
 * The spec surface is `spec new <slug>` (scaffold) + `task from-spec <path>`
 * (create task). The master plan's mention of "--from-file mode" is doc drift;
 * this script uses the verbs that exist.
 */

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { $ } from "bun";

// Minimal docs/architecture.yaml required by `task verify` so the architecture
// lint pass has a rules file to load. The lint_scope points at src/v2/**/*.ts
// which does not exist in the temp project, so 0 files are scanned, 0
// violations, and the verify exit is PASS.
const ARCHITECTURE_YAML = `version: 1
forward_only: true
layers:
  - types
  - config
  - repo
  - service
  - runtime
  - ui
cross_cutting:
  - providers
lint_scope:
  - "src/v2/**/*.ts"
passive_harness:
  forbidden_patterns:
    - setInterval
`;

// Resolve to an absolute path so the binary is still findable when the cwd
// changes to the temp project directory.
const MAESTRO_BIN = process.argv[2]
  ? resolve(process.cwd(), process.argv[2])
  : "maestro";

// ---- helpers ----------------------------------------------------------------

function step(label: string): void {
  console.log(`\n[v2-smoke] ${label}`);
}

function fail(msg: string): never {
  console.error(`\n[v2-smoke] FAIL: ${msg}`);
  process.exit(1);
}

async function run(args: string[], cwd: string): Promise<string> {
  const result =
    await $`${MAESTRO_BIN} ${args}`
      .cwd(cwd)
      .env({ ...process.env, MAESTRO_NO_UPDATE_CHECK: "1" })
      .quiet()
      .nothrow();

  const stdout = result.stdout.toString().trim();
  const stderr = result.stderr.toString().trim();

  if (result.exitCode !== 0) {
    const detail = [stdout, stderr].filter(Boolean).join("\n");
    fail(`\`maestro ${args.join(" ")}\` exited ${result.exitCode}\n${detail}`);
  }
  return stdout;
}

// Extract a token matching a pattern from multi-line output.
function extractToken(output: string, pattern: RegExp): string {
  const match = output.match(pattern);
  if (!match?.[1]) fail(`Could not extract token from output:\n${output}`);
  return match[1];
}

// ---- main -------------------------------------------------------------------

let tmpDir: string | undefined;

async function main(): Promise<void> {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-smoke-"));

  step(`Temp project: ${tmpDir}`);
  step("git init");
  await $`git init -b main`.cwd(tmpDir).quiet();

  step("Write docs/architecture.yaml (required by task verify)");
  await mkdir(join(tmpDir, "docs"), { recursive: true });
  await writeFile(join(tmpDir, "docs/architecture.yaml"), ARCHITECTURE_YAML);

  step("maestro setup bootstrap");
  await run(["setup", "bootstrap"], tmpDir);

  step("maestro setup check");
  const checkOut = await run(["setup", "check"], tmpDir);
  if (!checkOut.includes("setup check: OK") && !checkOut.includes("setup check: action required")) {
    fail(`Unexpected setup check output:\n${checkOut}`);
  }
  // Allow "action required" only if it comes from [warn] entries (no [miss] entries).
  if (checkOut.includes("[miss]")) {
    fail(`setup check reports missing items:\n${checkOut}`);
  }
  console.log(checkOut);

  step("maestro spec new smoke-spec");
  const specOut = await run(["spec", "new", "smoke-spec", "--title", "v2 Smoke Test"], tmpDir);
  console.log(specOut);
  // Confirm the spec file path was printed.
  if (!specOut.includes(".maestro/specs/smoke-spec.md")) {
    fail(`spec new did not print expected path:\n${specOut}`);
  }

  step("maestro task from-spec .maestro/specs/smoke-spec.md");
  const fromSpecOut = await run(
    ["task", "from-spec", ".maestro/specs/smoke-spec.md"],
    tmpDir,
  );
  console.log(fromSpecOut);
  const taskId = extractToken(fromSpecOut, /^(tsk-\S+)/m);
  console.log(`[v2-smoke] task ID: ${taskId}`);

  step(`maestro task claim ${taskId} --skip-worktree`);
  const claimOut = await run(["task", "claim", taskId, "--skip-worktree"], tmpDir);
  console.log(claimOut);
  if (!claimOut.includes("claimed")) {
    fail(`claim did not report claimed:\n${claimOut}`);
  }

  step(`maestro task verify ${taskId}`);
  const verifyOut = await run(["task", "verify", taskId], tmpDir);
  console.log(verifyOut);
  if (!verifyOut.includes("PASS")) {
    fail(`verify did not report PASS:\n${verifyOut}`);
  }

  step(`maestro task ship ${taskId}`);
  const shipOut = await run(["task", "ship", taskId], tmpDir);
  console.log(shipOut);
  if (!shipOut.includes("shipped")) {
    fail(`ship did not report shipped:\n${shipOut}`);
  }

  console.log("\n[v2-smoke] PASS: all v2 hot-path verbs succeeded");
}

main()
  .catch((err: unknown) => {
    console.error("[v2-smoke] unhandled error:", err);
    process.exit(1);
  })
  .finally(async () => {
    if (tmpDir) {
      await rm(tmpDir, { recursive: true, force: true }).catch(() => undefined);
    }
  });
