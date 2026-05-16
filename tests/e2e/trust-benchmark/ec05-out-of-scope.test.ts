/**
 * EC 5 — Out-of-scope harmless change.
 *
 * Mitigation: Trust Verifier scope check (L2.3).
 *
 * Positive: a file outside filesExpected triggers a scope error (exit 1).
 * Negative: a file inside filesExpected produces no scope error (exit 0 or 2).
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../../helpers/run-compiled-cli.js";
import { runCommand } from "../../helpers/command-runner.js";

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

const tempDirs: string[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
});

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec05-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);
  const init = await runCompiled(["init"], dir);
  if (init.exitCode !== 0) throw new Error(`maestro init failed: ${init.stderr}`);
  await runCommand(
    ["git", "commit", "--allow-empty", "-m", "init", "--author", "Test <test@example.com>"],
    dir,
  );
  return dir;
}

async function createTask(dir: string, title: string): Promise<string> {
  const r = await runCompiled(["task", "q", title], dir);
  if (r.exitCode !== 0) throw new Error(`task q failed: ${r.stderr}`);
  const id = r.stdout.trim();
  if (!id.match(/^tsk-[0-9a-f]{6}$/)) throw new Error(`Unexpected task id: "${id}"`);
  return id;
}

async function seedContract(
  dir: string,
  taskId: string,
  filesExpected: string[],
): Promise<void> {
  const contractDir = join(dir, ".maestro", "contracts", taskId);
  await mkdir(contractDir, { recursive: true });
  const contract = {
    schemaVersion: 2,
    id: `c-${taskId.slice(-6)}`,
    taskId,
    repoRoot: ".",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    lockedAt: "2026-01-01T00:00:01.000Z",
    intent: "EC05 scope test",
    scope: { filesExpected, filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec05-test",
    lockedBy: "ec05-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
    amendmentBudget: {
      maxAmendments: 4,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**"],
    },
  };
  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

async function commitFile(dir: string, relPath: string, content = "// test\n"): Promise<void> {
  const fullPath = join(dir, relPath);
  await mkdir(join(fullPath, ".."), { recursive: true });
  await writeFile(fullPath, content);
  await runCommand(["git", "add", relPath], dir);
  await runCommand(
    ["git", "commit", "-m", `chore: ${relPath}`, "--author", "Test <test@example.com>"],
    dir,
  );
}

// TODO(v2/phase-4): re-enable or remove. v1 `task verify` is detached per ADR-0007 big-bang;
// v2 equivalents live in src/runtime/task.command.ts.
describe.skip("EC 5 — out-of-scope harmless change (Trust Verifier scope check)", () => {
  it(
    "positive: file outside filesExpected produces scope error finding (exit 1)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "EC05 out-of-scope positive");
      // Contract only allows src/feature/**
      await seedContract(dir, taskId, ["src/feature/**"]);

      // Commit a file OUTSIDE scope
      await commitFile(dir, "src/other/harmless.ts", "export const x = 1;\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // Trust Verifier reports scope error → exit 1
      expect(result.exitCode).toBe(1);
      const output = expectJson<{
        findings: Array<{ check: string; severity: string; paths: string[] }>;
        counts: { error: number; warn: number; info: number };
      }>(result);
      expect(output.counts.error).toBeGreaterThan(0);
      const scopeError = output.findings.find(
        (f) => f.check === "scope" && f.severity === "error",
      );
      expect(scopeError).toBeDefined();
      expect(scopeError!.paths).toContain("src/other/harmless.ts");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: file inside filesExpected produces no scope error (exit 0 or 2)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "EC05 in-scope negative");
      await seedContract(dir, taskId, ["src/feature/**"]);

      // Commit a file INSIDE scope
      await commitFile(dir, "src/feature/impl.ts", "export const y = 2;\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // No scope error; exit 0 or 2 (warn/info level findings are OK)
      expect(result.exitCode).not.toBe(1);
      const output = expectJson<{
        findings: Array<{ check: string; severity: string }>;
        counts: { error: number; warn: number; info: number };
      }>(result);
      const scopeErrors = output.findings.filter(
        (f) => f.check === "scope" && f.severity === "error",
      );
      expect(scopeErrors).toHaveLength(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
