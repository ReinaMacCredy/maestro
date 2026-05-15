/**
 * EC 9 — Sensitive path access.
 *
 * Mitigation: sensitive-paths.yaml policy globs + Trust Verifier check (L2.3).
 *
 * Positive: file matching sensitive-paths glob → sensitive-paths warn finding.
 * Negative: file not matching any glob → no sensitive-paths finding.
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec09-"));
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

async function seedContract(dir: string, taskId: string, filesExpected: string[]): Promise<void> {
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
    intent: "EC09 sensitive path test",
    scope: { filesExpected, filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec09-test",
    lockedBy: "ec09-test",
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
      forbiddenAmendmentPaths: [],
    },
  };
  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

async function writeSensitivePaths(dir: string, globs: string[]): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  const lines = ["paths:", ...globs.map((g) => `  - "${g}"`)];
  await writeFile(join(policyDir, "sensitive-paths.yaml"), lines.join("\n"));
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
// v2 equivalents live in src/v2/runtime/task.command.ts.
describe.skip("EC 9 — sensitive path access (forbidden_paths + sensitive-paths.yaml)", () => {
  it(
    "positive: diff touching sensitive-paths glob emits sensitive-paths warn finding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Mark src/auth/** as sensitive
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "EC09 sensitive positive");
      await seedContract(dir, taskId, ["src/auth/**"]);

      await commitFile(dir, "src/auth/session.ts", "export function auth() {}\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      const output = expectJson<{
        findings: Array<{ check: string; severity: string; paths: string[] }>;
        counts: { error: number; warn: number; info: number };
      }>(result);

      const sensitiveWarn = output.findings.find(
        (f) => f.check === "sensitive-paths" && f.severity === "warn",
      );
      expect(sensitiveWarn).toBeDefined();
      expect(sensitiveWarn!.paths).toContain("src/auth/session.ts");
      // sensitive-paths is advisory warn, not error — exit 2 (warns present)
      expect(result.exitCode).toBe(2);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: diff not touching any sensitive glob produces no sensitive-paths finding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Mark src/auth/** as sensitive, but commit a non-auth file
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "EC09 sensitive negative");
      await seedContract(dir, taskId, ["src/ui/**"]);

      await commitFile(dir, "src/ui/button.ts", "export const btn = {};\n");

      const result = await runCompiled(
        ["task", "verify", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      const output = expectJson<{
        findings: Array<{ check: string; severity: string }>;
      }>(result);

      const sensitiveFindings = output.findings.filter((f) => f.check === "sensitive-paths");
      expect(sensitiveFindings).toHaveLength(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
