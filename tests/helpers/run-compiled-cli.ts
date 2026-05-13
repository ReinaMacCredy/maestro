/**
 * Test helpers for driving the compiled maestro CLI at dist/maestro.
 * Sibling to tests/helpers/run-cli.ts, which spawns TypeScript source.
 * e2e suites use this file to exercise the real binary a user would run.
 */
import { expect } from "bun:test";
import { join } from "node:path";
import {
  runCommand,
  initGitRepo,
  type CommandResult,
  type RunCommandOptions,
} from "./command-runner.js";

export const REPO_ROOT = join(import.meta.dir, "..", "..");
export const DIST_CLI = join(REPO_ROOT, "dist", process.platform === "win32" ? "maestro.exe" : "maestro");
export const BUILD_TIMEOUT_MS = 60_000;
export const SLOW_CLI_TIMEOUT_MS = 60_000;

export type { CommandResult };
export { initGitRepo };
export interface RunCompiledOptions extends RunCommandOptions {}

/**
 * Spawn the compiled dist/maestro binary with the given args and capture
 * stdout / stderr / exit code. Returns a CommandResult with trimmed output.
 *
 * Matches the ergonomics of `runCli` (tests/helpers/run-cli.ts) for
 * easy refactoring between source-based and compiled-based suites.
 */
export async function runCompiled(
  args: readonly string[],
  cwd: string = process.cwd(),
  options: RunCompiledOptions = {},
): Promise<CommandResult> {
  return runCommand([DIST_CLI, ...args], cwd, options);
}

/**
 * Build the compiled CLI once at the top of a test file. Asserts exit 0.
 * Use inside `beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS)` so the
 * binary is fresh for the entire suite.
 *
 * The build is cached per-process: repeated calls across e2e test files
 * reuse the first result. This avoids hammering Windows with repeated
 * `rename dist/maestro.exe` calls that intermittently fail with EPERM
 * due to transient file locks (antivirus / lingering handles).
 *
 * OPTIMIZATION: Checks if binary exists and is recent (< 5 min old) before
 * rebuilding. This allows `pretest` script to build once, and all test
 * processes can skip the rebuild if the binary is fresh.
 */
let buildPromise: Promise<void> | null = null;
export async function buildCompiledCli(): Promise<void> {
  if (!buildPromise) {
    buildPromise = (async () => {
      // Check if binary exists and is recent (< 5 min old)
      try {
        const file = Bun.file(DIST_CLI);
        const exists = await file.exists();
        if (exists) {
          const stat = await file.stat();
          const ageMs = Date.now() - stat.mtime.getTime();
          const FRESHNESS_THRESHOLD_MS = 5 * 60 * 1000; // 5 minutes
          if (ageMs < FRESHNESS_THRESHOLD_MS) {
            // Binary is fresh, skip rebuild
            return;
          }
        }
      } catch {
        // Binary doesn't exist or stat failed, need to build
      }
      
      // Binary is stale or missing, rebuild
      const result = await runCommand(["bun", "run", "build"], REPO_ROOT);
      expect(result).toMatchObject({ exitCode: 0 });
    })().catch((err) => {
      buildPromise = null;
      throw err;
    });
  }
  return buildPromise;
}

/**
 * Parse JSON from a compiled CLI stdout, with a helpful error message
 * that includes stdout + stderr if parsing fails. Tests should prefer
 * this over bare `JSON.parse(result.stdout)` so mismatches surface
 * the actual command output in the failure message.
 */
export function expectJson<T = unknown>(result: CommandResult): T {
  try {
    return JSON.parse(result.stdout) as T;
  } catch (err) {
    throw new Error(
      `Failed to parse JSON from compiled CLI stdout.\n` +
        `stdout: ${result.stdout}\n` +
        `stderr: ${result.stderr}\n` +
        `exit code: ${result.exitCode}\n` +
        `parse error: ${(err as Error).message}`,
    );
  }
}
