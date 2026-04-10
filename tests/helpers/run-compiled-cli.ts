/**
 * Test helpers for driving the compiled maestro CLI at dist/maestro.
 * Sibling to tests/helpers/run-cli.ts, which spawns TypeScript source.
 * e2e suites use this file to exercise the real binary a user would run.
 */
import { expect } from "bun:test";
import { join } from "node:path";

export const REPO_ROOT = join(import.meta.dir, "..", "..");
export const DIST_CLI = join(REPO_ROOT, "dist", "maestro");
export const BUILD_TIMEOUT_MS = 60_000;
export const SLOW_CLI_TIMEOUT_MS = 30_000;

export interface CommandResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

export interface RunCompiledOptions {
  /** Extra environment variables to inject (merged over process.env). */
  readonly env?: Record<string, string>;
  /** Stdin bytes or string to write to the process before reading output. */
  readonly stdin?: string;
}

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
  const proc = Bun.spawn([DIST_CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    stdin: options.stdin !== undefined ? "pipe" : "inherit",
    cwd,
    env: options.env ? { ...process.env, ...options.env } : process.env,
  });

  if (options.stdin !== undefined && proc.stdin) {
    proc.stdin.write(options.stdin);
    await proc.stdin.end();
  }

  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode: await proc.exited,
  };
}

/**
 * Build the compiled CLI once at the top of a test file. Asserts exit 0.
 * Use inside `beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS)` so the
 * binary is fresh for the entire suite.
 *
 * Subsequent runs of `bun run build` are cheap (~200ms bundle + compile)
 * because Bun caches unchanged modules.
 */
export async function buildCompiledCli(): Promise<void> {
  const proc = Bun.spawn(["bun", "run", "build"], {
    cwd: REPO_ROOT,
    stdout: "pipe",
    stderr: "pipe",
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  expect({ stdout, stderr, exitCode }).toMatchObject({ exitCode: 0 });
}

/**
 * Initialize an empty git repo in the given directory. Many maestro
 * commands short-circuit when they cannot find a git branch, so every
 * e2e test that writes to `.maestro/` should call this from beforeEach.
 */
export async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
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
