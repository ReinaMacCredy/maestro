/**
 * EC 22 — Amendments hide scope creep.
 *
 * Mitigation: Amendment-budget rules 3–7 at L2 (amend-contract.usecase.ts).
 *
 * The budget enforces:
 *   - maxAmendments: total amendment count cap
 *   - maxPathsPerAmendment: paths-per-amendment cap
 *   - forbiddenAmendmentPaths: glob patterns that may never be added via amendment
 *
 * Positive: exceeding maxAmendments triggers a "budget_exhausted" block (exit non-0).
 * Negative: first amendment within budget succeeds (exit 0, newVersion=2).
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec22-"));
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
  maxAmendments: number,
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
    intent: "EC22 amendment creep test",
    scope: { filesExpected: ["src/feature/**"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec22-test",
    lockedBy: "ec22-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
    amendmentBudget: {
      maxAmendments,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**"],
    },
  };
  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

describe("EC 22 — amendment creep (amendment budget rules 3–7)", () => {
  it(
    "positive: exceeding maxAmendments budget blocks further amendments (exit non-0)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "EC22 budget exhausted");
      // maxAmendments=1 means only one amendment allowed
      await seedContract(dir, taskId, 1);

      // First amendment — should succeed
      const first = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "src/extra/a.ts",
          "--reason", "EC22 first amendment",
          "--json",
        ],
        dir,
      );
      expect(first.exitCode).toBe(0);
      const firstResult = expectJson<{ amendmentId: string; newVersion: number }>(first);
      expect(firstResult.newVersion).toBe(2);

      // Second amendment — budget exhausted (maxAmendments=1, already used 1)
      const second = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "src/extra/b.ts",
          "--reason", "EC22 second amendment (over budget)",
          "--json",
        ],
        dir,
      );
      expect(second.exitCode).not.toBe(0);
      const combined = second.stdout + " " + second.stderr;
      expect(combined).toMatch(/budget.*exhausted|exhausted.*budget|Amendment budget/i);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: first amendment within budget succeeds (exit 0, newVersion=2)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "EC22 within budget");
      await seedContract(dir, taskId, 4); // generous budget

      const result = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "src/extra/c.ts",
          "--reason", "EC22 valid amendment",
          "--json",
        ],
        dir,
      );
      expect(result.exitCode).toBe(0);
      const amended = expectJson<{ amendmentId: string; newVersion: number }>(result);
      expect(amended.amendmentId).toMatch(/^a-[0-9a-f]{6}$/);
      expect(amended.newVersion).toBe(2);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
