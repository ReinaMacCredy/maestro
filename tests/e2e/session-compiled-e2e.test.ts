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

// Force the session adapter's "no agent" branch even when the test runner
// itself is invoked from inside Claude Code or Codex.
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
      // --json mode routes MaestroError to stdout instead of stderr.
      expect(result.exitCode).not.toBe(0);
      const payload = expectJson<{ error: string; hints: string[] }>(result);
      expect(payload.error).toBe("No session detected");
      expect(Array.isArray(payload.hints)).toBe(true);
      expect(payload.hints.length).toBeGreaterThan(0);
      const joined = payload.hints.join(" ");
      expect(joined).toContain("CLAUDECODE");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "-q / --quiet parses cleanly and exits silently on failure",
    async () => {
      const result = await runCompiled(["session", "-q"], tmpDir, {
        env: NO_AGENT_ENV,
      });
      expect(result.exitCode).not.toBe(0);
      // commander emits "unknown option" only when the flag is not wired —
      // this is the regression guard for the -q / --quiet shorthand.
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).not.toContain("unknown option");
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
