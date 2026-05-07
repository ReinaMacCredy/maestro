/**
 * EC 6 — Generated-file drift.
 *
 * Mitigation: Generated-file parity check at L2.3 (Trust Verifier).
 *
 * The check reads package.json scripts looking for `sync:*` keys. When found,
 * it emits an info-level finding noting that generators may not have been run.
 * It never errors — the check is advisory only.
 *
 * Positive: project with sync:* scripts → generated-file-parity finding present.
 * Negative: project without sync:* scripts → no generated-file-parity finding.
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec06-"));
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

async function seedContract(dir: string, taskId: string): Promise<void> {
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
    intent: "EC06 generated drift test",
    scope: { filesExpected: ["src/**"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec06-test",
    lockedBy: "ec06-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "low",
    amendmentBudget: {
      maxAmendments: 4,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: [],
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

describe("EC 6 — generated-file drift (generated-file-parity check)", () => {
  it(
    "positive: package.json with sync:* scripts emits generated-file-parity finding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Write a package.json with a sync: script
      const pkg = {
        name: "test-project",
        version: "1.0.0",
        scripts: {
          "sync:templates": "node scripts/sync.js",
          build: "bun run build.ts",
        },
      };
      await writeFile(join(dir, "package.json"), JSON.stringify(pkg, null, 2));

      const taskId = await createTask(dir, "EC06 drift positive");
      await seedContract(dir, taskId);

      // Commit a regular source file — the generated-file-parity check reads
      // package.json from the project root (not from the diff).
      await commitFile(dir, "src/foo.ts", "export const x = 1;\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // Must include the generated-file-parity finding (info severity)
      const output = expectJson<{
        findings: Array<{ check: string; severity: string; details?: string }>;
        counts: { error: number; warn: number; info: number };
      }>(result);

      const genFinding = output.findings.find((f) => f.check === "generated-file-parity");
      expect(genFinding).toBeDefined();
      expect(genFinding!.severity).toBe("info");
      // Details should mention the detected sync script
      expect(genFinding!.details).toContain("sync:templates");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: package.json without sync:* scripts produces no generated-file-parity finding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Write a package.json WITHOUT any sync: scripts
      const pkg = {
        name: "test-project",
        version: "1.0.0",
        scripts: {
          build: "bun run build.ts",
          test: "bun test",
        },
      };
      await writeFile(join(dir, "package.json"), JSON.stringify(pkg, null, 2));

      const taskId = await createTask(dir, "EC06 drift negative");
      await seedContract(dir, taskId);

      await commitFile(dir, "src/bar.ts", "export const y = 2;\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      const output = expectJson<{
        findings: Array<{ check: string; severity: string }>;
      }>(result);

      const genFinding = output.findings.find((f) => f.check === "generated-file-parity");
      expect(genFinding).toBeUndefined();
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
