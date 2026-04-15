import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
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

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-ratchet-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function seedCorrection(
  cwd: string,
  rule = "use bun not npm",
  trigger = "npm,install",
): Promise<string> {
  const result = await runCompiled(
    [
      "memory-correct",
      rule,
      "--trigger",
      trigger,
      "--severity",
      "hard",
      "--json",
    ],
    cwd,
  );
  expect(result.exitCode).toBe(0);
  const correction = expectJson<{ id: string }>(result);
  return correction.id;
}

describe("compiled ratchet feature E2E", () => {
  it(
    "ratchet-check on empty state returns zero assertions and passed=true",
    async () => {
      const result = await runCompiled(["ratchet-check", "--json"], tmpDir);
      expect(result.exitCode).toBe(0);
      const payload = expectJson<{
        results: unknown[];
        totalCount: number;
        passCount: number;
        passed: boolean;
      }>(result);
      expect(payload.results).toEqual([]);
      expect(payload.totalCount).toBe(0);
      expect(payload.passCount).toBe(0);
      expect(payload.passed).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "ratchet-promote requires a valid correction id",
    async () => {
      // Reference a correction id that does not exist.
      const result = await runCompiled(
        [
          "ratchet-promote",
          "2026-01-01-999",
          "--check",
          "npm install",
        ],
        tmpDir,
      );
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      // Must reference "correction" or "not found" in the error.
      expect(combined.toLowerCase()).toMatch(/correction|not found/);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "promote then check reports passing when no files match the regex",
    async () => {
      const correctionId = await seedCorrection(tmpDir);

      const promote = await runCompiled(
        [
          "ratchet-promote",
          correctionId,
          "--check",
          "npm install",
          "--json",
        ],
        tmpDir,
      );
      expect(promote.exitCode).toBe(0);
      const promoted = expectJson<{
        assertion: {
          id: string;
          correctionId: string;
          rule: string;
          check: string;
        };
      }>(promote);
      expect(promoted.assertion.id).toContain(correctionId);
      expect(promoted.assertion.correctionId).toBe(correctionId);
      expect(promoted.assertion.check).toBe("npm install");
      expect(promoted.assertion.rule).toBe("use bun not npm");

      // Nothing in the workspace matches /npm install/, so check passes.
      const check = await runCompiled(["ratchet-check", "--json"], tmpDir);
      expect(check.exitCode).toBe(0);
      const checked = expectJson<{
        results: Array<{ passed: boolean; assertion: { id: string } }>;
        passCount: number;
        totalCount: number;
        passed: boolean;
        }>(check);
        expect(checked.totalCount).toBe(1);
        expect(checked.passCount).toBe(1);
        expect(checked.passed).toBe(true);
        expect(checked.results[0]?.passed).toBe(true);
        expect(checked.results[0]?.assertion.id).toBe(promoted.assertion.id);
      },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "check reports a violation when a file matches the ratchet regex",
    async () => {
      const correctionId = await seedCorrection(tmpDir);

      const promote = await runCompiled(
        [
          "ratchet-promote",
          correctionId,
          "--check",
          "npm install",
          "--json",
        ],
        tmpDir,
      );
      expect(promote.exitCode).toBe(0);

      await writeFile(
        join(tmpDir, "violation.sh"),
        "#!/bin/sh\nnpm install something\n",
      );

      const check = await runCompiled(["ratchet-check", "--json"], tmpDir);
      // ratchet-check is diagnostic: exit 0 even when passed=false.
      expect(check.exitCode).toBe(0);
      const checked = expectJson<{
        results: Array<{ passed: boolean; detail?: string }>;
        passCount: number;
        totalCount: number;
        passed: boolean;
      }>(check);
      expect(checked.totalCount).toBe(1);
      expect(checked.passCount).toBe(0);
      expect(checked.passed).toBe(false);
      expect(checked.results[0]?.passed).toBe(false);
      expect(checked.results[0]?.detail).toContain("violation");
      expect(checked.results[0]?.detail).toContain("violation.sh");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "ratchet-check records a baseline on second invocation",
    async () => {
      const correctionId = await seedCorrection(tmpDir);
      await runCompiled(
        [
          "ratchet-promote",
          correctionId,
          "--check",
          "npm install",
          "--json",
        ],
        tmpDir,
      );

      const first = await runCompiled(["ratchet-check", "--json"], tmpDir);
      expect(first.exitCode).toBe(0);

      const second = await runCompiled(["ratchet-check", "--json"], tmpDir);
      expect(second.exitCode).toBe(0);
      const payload = expectJson<{
        previousBaseline?: { passCount: number; lastRunAt: string };
      }>(second);
      expect(payload.previousBaseline).toBeDefined();
      expect(payload.previousBaseline?.lastRunAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "ratchet-check exit code is always 0 (diagnostic, not a gate)",
    async () => {
      // Even when violations exist, exit code stays 0. This is intentional
      // — ratchet is a reporting tool, not a CI gate. If that changes,
      // this test must be updated.
      const correctionId = await seedCorrection(tmpDir);
      await runCompiled(
        [
          "ratchet-promote",
          correctionId,
          "--check",
          "npm install",
          "--json",
        ],
        tmpDir,
      );
      await writeFile(join(tmpDir, "bad.txt"), "npm install broke me");

      const check = await runCompiled(["ratchet-check"], tmpDir);
      expect(check.exitCode).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
