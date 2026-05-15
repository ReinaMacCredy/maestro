/**
 * L2.E2E — Full L2 contract flow against ./dist/maestro.
 *
 * Exercises: init -> task add -> contract seed (v1.json) -> contract show ->
 * allowed-path diff (exit 0 or 2) -> forbidden-path diff (exit 1) ->
 * non-sensitive amend (success, v2) -> sensitive amend (blocked, evidence row).
 *
 * Per ROADMAP.md L2.E2E.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, readdir, rm, writeFile } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-l2-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

/** Write a minimal but fully-valid v1.json contract for the given taskId. */
async function seedV1Contract(tmpDir: string, taskId: string): Promise<void> {
  const contractDir = join(tmpDir, ".maestro", "contracts", taskId);
  await mkdir(contractDir, { recursive: true });

  const contract = {
    schemaVersion: 2,
    id: "c-a1b2c3",
    taskId,
    repoRoot: ".",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    lockedAt: "2026-01-01T00:00:01.000Z",
    intent: "Implement feature-x",
    scope: {
      filesExpected: ["src/feature-x/**", "tests/feature-x/**"],
      filesForbidden: [".github/workflows/**", "bun.lock"],
    },
    doneWhen: [
      {
        id: "dw-000001",
        text: "Feature-x implemented and tests passing",
        kind: "manual",
      },
    ],
    amendments: [],
    createdBy: "l2-e2e-test",
    lockedBy: "l2-e2e-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
    amendmentBudget: {
      maxAmendments: 2,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**", "package.json"],
    },
  };

  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}


// TODO(v2/phase-4): re-enable or remove. v1 `task verify` is detached per ADR-0007 big-bang;
// v2 equivalents live in src/v2/runtime/task.command.ts.
describe.skip("L2 contract flow E2E", () => {
  it(
    "init, task add, contract seed, allowed-path verify, forbidden-path verify, amend (success), amend (blocked)",
    async () => {
      // ── 1. maestro init ────────────────────────────────────────────────────
      const initResult = await runCompiled(["init"], tmpDir);
      expect(initResult.exitCode).toBe(0);

      // ── 2. Create a task ───────────────────────────────────────────────────
      const taskResult = await runCompiled(
        ["task", "q", "L2 contract E2E test task"],
        tmpDir,
      );
      expect(taskResult.exitCode).toBe(0);
      const taskId = taskResult.stdout.trim();
      expect(taskId).toMatch(/^tsk-[0-9a-f]{6}$/);

      // ── 3. Seed v1 contract ────────────────────────────────────────────────
      await seedV1Contract(tmpDir, taskId);

      // ── 4. contract show --json ────────────────────────────────────────────
      const showResult = await runCompiled(
        ["contract", "show", "--task", taskId, "--json"],
        tmpDir,
      );
      expect(showResult.exitCode).toBe(0);
      const shown = expectJson<{
        id: string;
        taskId: string;
        status: string;
        riskClass: string;
        scope: { filesExpected: string[]; filesForbidden: string[] };
        amendmentBudget: {
          maxAmendments: number;
          maxPathsPerAmendment: number;
          forbiddenAmendmentPaths: string[];
        };
      }>(showResult);
      expect(shown.id).toBe("c-a1b2c3");
      expect(shown.taskId).toBe(taskId);
      expect(shown.status).toBe("locked");
      expect(shown.riskClass).toBe("medium");
      expect(shown.scope.filesExpected).toEqual(["src/feature-x/**", "tests/feature-x/**"]);
      expect(shown.scope.filesForbidden).toEqual([".github/workflows/**", "bun.lock"]);
      expect(shown.amendmentBudget.maxAmendments).toBe(2);
      expect(shown.amendmentBudget.maxPathsPerAmendment).toBe(3);
      expect(shown.amendmentBudget.forbiddenAmendmentPaths).toEqual(["**/secrets/**", "package.json"]);

      // ── 5. Allowed-path diff: touches src/feature-x/foo.ts ─────────────────
      // Create an initial commit so HEAD exists
      const { runCommand } = await import("../helpers/command-runner.js");
      await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
      await runCommand(["git", "config", "user.name", "Test"], tmpDir);

      // Initial empty commit so HEAD is valid
      await runCommand(["git", "commit", "--allow-empty", "-m", "init", "--author", "Test <test@example.com>"], tmpDir);

      // Commit an allowed file
      const allowedFile = join(tmpDir, "src", "feature-x", "foo.ts");
      await mkdir(join(tmpDir, "src", "feature-x"), { recursive: true });
      await writeFile(allowedFile, "export const x = 1;\n");
      await runCommand(["git", "add", "src/feature-x/foo.ts"], tmpDir);
      await runCommand(
        ["git", "commit", "-m", "feat: add allowed file", "--author", "Test <test@example.com>"],
        tmpDir,
      );

      const verifyAllowedResult = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        tmpDir,
      );

      // Exit 0 means no findings. Exit 2 means only warn/info. Both are acceptable
      // for an allowed path (no errors). Exit 1 means errors — that would be a failure.
      expect(verifyAllowedResult.exitCode).not.toBe(1);
      const allowedFindings = expectJson<{
        findings: Array<{ check: string; severity: string }>;
        counts: { error: number; warn: number; info: number };
      }>(verifyAllowedResult);
      expect(allowedFindings.counts.error).toBe(0);

      // ── 6. Forbidden-path diff: touches .github/workflows/ci.yml ───────────
      const forbiddenFile = join(tmpDir, ".github", "workflows", "ci.yml");
      await mkdir(join(tmpDir, ".github", "workflows"), { recursive: true });
      await writeFile(forbiddenFile, "name: CI\n");
      await runCommand(["git", "add", ".github/workflows/ci.yml"], tmpDir);
      await runCommand(
        ["git", "commit", "-m", "chore: add CI workflow", "--author", "Test <test@example.com>"],
        tmpDir,
      );

      const verifyForbiddenResult = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        tmpDir,
      );
      expect(verifyForbiddenResult.exitCode).toBe(1);
      const forbiddenFindings = expectJson<{
        findings: Array<{ check: string; severity: string; paths: string[] }>;
        counts: { error: number; warn: number; info: number };
      }>(verifyForbiddenResult);
      expect(forbiddenFindings.counts.error).toBeGreaterThan(0);
      const scopeError = forbiddenFindings.findings.find(
        (f) => f.severity === "error" && f.check === "scope",
      );
      expect(scopeError).toBeDefined();

      // ── 7. Amend: non-sensitive path (should succeed) ───────────────────────
      const amendResult = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "src/extra/**",
          "--reason", "discovered during impl",
          "--json",
        ],
        tmpDir,
      );
      expect(amendResult.exitCode).toBe(0);
      const amended = expectJson<{ amendmentId: string; newVersion: number }>(amendResult);
      expect(amended.amendmentId).toMatch(/^a-[0-9a-f]{6}$/);
      expect(amended.newVersion).toBe(2);

      // ── 8. Amend: forbidden path (should be blocked) ────────────────────────
      const amendForbiddenResult = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "package.json",
          "--reason", "tries to write a forbidden path",
          "--json",
        ],
        tmpDir,
      );
      expect(amendForbiddenResult.exitCode).not.toBe(0);
      // The error message should mention "forbidden"
      const combinedOutput = amendForbiddenResult.stdout + "\n" + amendForbiddenResult.stderr;
      expect(combinedOutput).toMatch(/forbidden/i);

      // ── 9. Verify evidence row of kind contract-amendment-blocked was written
      const evidenceDir = join(tmpDir, ".maestro", "evidence", taskId);
      const evidenceFiles = await readdir(evidenceDir).catch(() => [] as string[]);
      // Read each evidence file and check for the blocked kind
      let foundBlocked = false;
      for (const file of evidenceFiles) {
        if (!file.endsWith(".json")) continue;
        const raw = await import("node:fs/promises").then((fs) =>
          fs.readFile(join(evidenceDir, file), "utf8"),
        );
        const row = JSON.parse(raw) as { kind: string };
        if (row.kind === "contract-amendment-blocked") {
          foundBlocked = true;
          break;
        }
      }
      expect(foundBlocked).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
