/**
 * End-to-end test for the session feature against ./dist/maestro.
 *
 * The session feature is env-only: it reads CLAUDECODE and CODEX_THREAD_ID
 * and matches them against on-disk agent session files in ~/.claude or
 * ~/.codex. We cannot reliably plant those files from a test tmpdir, so
 * this e2e focuses on the CLI surface:
 *  - exit code and error shape when no agent env is present
 *  - --json output structure for the error path
 *  - -q / --quiet flag still runs the command
 *  - help text includes the expected examples
 *
 * When the suite happens to run inside a real agent environment (CLAUDECODE
 * or CODEX_THREAD_ID set in the parent shell), the tests accept the
 * detected-session path as a pass too — both branches are valid evidence
 * that the wiring is correct.
 */
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

/**
 * Env that forces the session adapter into the "no agent" branch regardless
 * of the test runner's parent environment. We blank out both detection
 * env vars so the test is deterministic even when the suite is invoked
 * from inside Claude Code itself.
 */
const NO_AGENT_ENV = {
  CLAUDECODE: "",
  CODEX_THREAD_ID: "",
};

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-session-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled session feature E2E", () => {
  it(
    "exits non-zero with a MaestroError when no agent env is present",
    async () => {
      const result = await runCompiled(["session"], tmpDir, {
        env: NO_AGENT_ENV,
      });
      expect(result.exitCode).not.toBe(0);
      // The MaestroError handler in src/index.ts writes to stderr for text
      // output and to stdout for --json. Both paths must mention the
      // detection problem.
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).toContain("No session detected");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "--json returns a structured error with hints when no agent env",
    async () => {
      const result = await runCompiled(["session", "--json"], tmpDir, {
        env: NO_AGENT_ENV,
      });
      // --json mode puts the MaestroError on stdout, not stderr.
      expect(result.exitCode).not.toBe(0);
      const payload = expectJson<{ error: string; hints: string[] }>(result);
      expect(payload.error).toBe("No session detected");
      expect(Array.isArray(payload.hints)).toBe(true);
      expect(payload.hints.length).toBeGreaterThan(0);
      // The hints should at least name the env vars maestro reads.
      const joined = payload.hints.join(" ");
      expect(joined).toContain("CLAUDECODE");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "-q / --quiet parses cleanly and exits silently on failure",
    async () => {
      // --quiet is a scripting aid: on success it prints only the session
      // id; on failure it exits non-zero with empty output. This test is
      // a flag-wiring regression guard — if -q is removed or renamed,
      // commander will emit "unknown option" and this test catches it.
      const result = await runCompiled(["session", "-q"], tmpDir, {
        env: NO_AGENT_ENV,
      });
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).not.toContain("unknown option");
      // stdout must be empty in quiet mode (no "No session detected" text).
      expect(result.stdout).toBe("");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "--help lists the expected flags and examples",
    async () => {
      const result = await runCompiled(["session", "--help"], tmpDir);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("Detect the current agent session");
      expect(result.stdout).toContain("--json");
      expect(result.stdout).toContain("-q, --quiet");
      expect(result.stdout).toContain("Examples:");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
