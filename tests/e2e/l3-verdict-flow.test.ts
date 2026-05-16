/**
 * L3.E2E — Full L3 verdict flow against ./dist/maestro.
 *
 * Covers:
 *   PASS   — clean diff + medium risk + permissive autopilot → exit 0
 *   FAIL   — forbidden-path scope violation → exit 1
 *   HUMAN  — agent proposes low for src/auth/** → effective critical, exit 2
 *            (canonical Rule 1 gameability case)
 *   ProofMap — 3 criteria, 2 evidence rows, 1 uncovered
 *   BLOCK  — deferred to L4.4 (cost-budget reader not wired)
 *
 * Per ROADMAP.md L3.E2E.
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
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

// ─── shared helpers ───────────────────────────────────────────────────────────

/**
 * Bootstrap a clean temp git repo with maestro init.
 * Returns the path to the temp dir.
 */
async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l3-e2e-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);

  const initResult = await runCompiled(["init"], dir);
  if (initResult.exitCode !== 0) {
    throw new Error(`maestro init failed: ${initResult.stderr || initResult.stdout}`);
  }

  // Create initial commit so HEAD is valid for diff resolution.
  await runCommand(
    ["git", "commit", "--allow-empty", "-m", "init", "--author", "Test <test@example.com>"],
    dir,
  );

  return dir;
}

/**
 * Create a task via `task q` and return the task id.
 */
async function createTask(dir: string, title: string): Promise<string> {
  const result = await runCompiled(["task", "q", title], dir);
  if (result.exitCode !== 0) {
    throw new Error(`task q failed: ${result.stderr || result.stdout}`);
  }
  const taskId = result.stdout.trim();
  if (!taskId.match(/^tsk-[0-9a-f]{6}$/)) {
    throw new Error(`Unexpected task id: ${taskId}`);
  }
  return taskId;
}

/**
 * Seed a minimal locked contract for the given task. The scope controls which
 * paths are allowed; riskClass is the agent-proposed value.
 */
async function seedContract(
  dir: string,
  taskId: string,
  opts: {
    filesExpected: string[];
    riskClass: string;
    maxAmendments?: number;
  },
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
    intent: "L3 e2e test",
    scope: {
      filesExpected: opts.filesExpected,
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "l3-e2e-test",
    lockedBy: "l3-e2e-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: opts.riskClass,
    amendmentBudget: {
      maxAmendments: opts.maxAmendments ?? 2,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**"],
    },
  };

  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

/**
 * Write an autopilot policy that allows auto-merge for medium risk (needed for
 * PASS scenario). Without this the default policy blocks auto-merge for all
 * risk classes and every verdict would be HUMAN via rule 6.
 */
async function writeMediumPermissiveAutopilot(dir: string): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  await writeFile(
    join(policyDir, "autopilot.yaml"),
    [
      "kind: autopilot",
      "id: autopilot-policy-test",
      "version: \"1\"",
      "auto_merge_allowed:",
      "  low: true",
      "  medium: true",
      "  high: false",
      "  critical: false",
      "required_witness_level:",
      "  low: agent-claimed-locally",
      "  medium: agent-claimed-locally",
      "  high: witnessed-by-maestro",
      "  critical: witnessed-by-maestro",
    ].join("\n"),
  );
}

async function writeSensitivePaths(dir: string, globs: string[]): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  const lines = ["paths:"];
  for (const g of globs) {
    lines.push(`  - "${g}"`);
  }
  await writeFile(join(policyDir, "sensitive-paths.yaml"), lines.join("\n"));
}

/**
 * Commit a file to the repo so it appears in the diff relative to HEAD~1.
 */
async function commitFile(dir: string, relPath: string, content = "// test\n"): Promise<void> {
  const fullPath = join(dir, relPath);
  await mkdir(join(dir, relPath, ".."), { recursive: true });
  await writeFile(fullPath, content);
  await runCommand(["git", "add", relPath], dir);
  await runCommand(
    ["git", "commit", "-m", `chore: add ${relPath}`, "--author", "Test <test@example.com>"],
    dir,
  );
}

// ─── teardown tracking ────────────────────────────────────────────────────────

const tempDirs: string[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
});

// ─── scenarios ────────────────────────────────────────────────────────────────

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("L3 verdict flow (compiled binary)", () => {
  it(
    "PASS: clean diff + strong evidence → exit 0, decision PASS",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Allow auto-merge for medium risk (default policy blocks all auto-merge).
      await writeMediumPermissiveAutopilot(dir);

      // No sensitive-paths match for src/foo.ts, so derivedRiskClass stays medium.
      // Do NOT write sensitive-paths.yaml for src/foo.ts — leave it absent so
      // the loader returns [] and the signal diff-intersects-sensitive-security
      // does not fire.

      const taskId = await createTask(dir, "PASS scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/foo.ts"],
        riskClass: "medium",
      });

      // Commit the file in scope. Verdict diff base is HEAD~1.
      await commitFile(dir, "src/foo.ts", "export const x = 1;\n");

      // Record a command-evidence row (no criterion needed — no Spec linked).
      const evResult = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--command", "echo ok",
          "--exit", "0",
          "--json",
        ],
        dir,
      );
      expect(evResult.exitCode).toBe(0);

      // Request verdict with explicit base so the diff is deterministic.
      const result = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);
      const verdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        proposedRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(result);
      expect(verdict.decision).toBe("PASS");
      expect(verdict.effectiveRiskClass).toBe("medium");
      expect(verdict.proposedRiskClass).toBe("medium");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "FAIL: forbidden-path scope violation → exit 1, decision FAIL",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "FAIL scenario");
      // Contract scope only allows src/foo.ts but we will touch src/forbidden.ts.
      await seedContract(dir, taskId, {
        filesExpected: ["src/foo.ts"],
        riskClass: "medium",
      });

      // Commit an allowed file, then commit a forbidden file.
      // Verdict base will be HEAD~1 so the diff sees only src/forbidden.ts.
      await commitFile(dir, "src/foo.ts", "export const x = 1;\n");
      await commitFile(dir, "src/forbidden.ts", "export const y = 2;\n");

      const result = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      const verdict = expectJson<{
        decision: string;
        reasons: Array<{ category: string; code: string; findingChecks?: string[] }>;
        trustVerifier: { errors: number };
      }>(result);
      expect(verdict.decision).toBe("FAIL");
      expect(verdict.trustVerifier.errors).toBeGreaterThan(0);
      const trustReason = verdict.reasons.find((r) => r.category === "trust");
      expect(trustReason).toBeDefined();
      // The scope check should appear in findingChecks.
      expect(trustReason!.findingChecks).toContain("scope");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "HUMAN: agent proposes risk_class=low for src/auth/** → effective critical, exit 2",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "HUMAN scenario");
      // Agent proposes low — the canonical Rule 1 gameability case.
      await seedContract(dir, taskId, {
        filesExpected: ["src/auth/**"],
        riskClass: "low",
      });

      // Touch src/auth/handler.ts — matches the sensitive-paths glob.
      await commitFile(dir, "src/auth/handler.ts", "export function login() {}\n");

      const result = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // derivedRiskClass = critical (diff-intersects-sensitive-security fired)
      // effectiveRiskClass = max(low, critical) = critical → HUMAN (rule 4)
      expect(result.exitCode).toBe(2);
      const verdict = expectJson<{
        decision: string;
        proposedRiskClass: string;
        effectiveRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(result);
      expect(verdict.decision).toBe("HUMAN");
      expect(verdict.proposedRiskClass).toBe("low");
      expect(verdict.effectiveRiskClass).toBe("critical");
      const riskReason = verdict.reasons.find((r) => r.category === "risk");
      expect(riskReason).toBeDefined();
      expect(riskReason!.code).toBe("effective-risk-critical");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "ProofMap: spec with 3 criteria + 2 evidence rows → 1 uncovered",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "ProofMap scenario");

      // Write a Spec directly (spec edit requires $EDITOR — bypass via file write).
      // The task needs a missionId so the proof command resolves the Spec.
      // Use a synthetic missionId that doesn't need to exist as a mission object;
      // specStore only requires the file under .maestro/specs/<missionId>.json.
      const missionId = "2026-05-04-001";

      // Link the task to the mission by patching tasks.jsonl.
      const tasksFile = join(dir, ".maestro", "tasks", "tasks.jsonl");
      const { readFile } = await import("node:fs/promises");
      let tasksContent = "";
      try {
        tasksContent = await readFile(tasksFile, "utf8");
      } catch {
        // file may not exist yet
      }
      // Replace the task's missionId field. The task was created via task q so
      // it has no missionId. Parse + patch each line.
      const patchedLines = tasksContent
        .split("\n")
        .filter((l) => l.trim().length > 0)
        .map((line) => {
          try {
            const obj = JSON.parse(line) as Record<string, unknown>;
            if (obj["id"] === taskId) {
              obj["missionId"] = missionId;
            }
            return JSON.stringify(obj);
          } catch {
            return line;
          }
        });
      await writeFile(tasksFile, patchedLines.join("\n") + "\n");

      // Write the Spec with 3 acceptance criteria.
      const specsDir = join(dir, ".maestro", "specs");
      await mkdir(specsDir, { recursive: true });
      const now = new Date().toISOString();
      const spec = {
        schema_version: 1,
        mission_id: missionId,
        acceptance_criteria: [
          { id: "crit-001", text: "Feature A works" },
          { id: "crit-002", text: "Feature B works" },
          { id: "crit-003", text: "Feature C works" },
        ],
        non_goals: [],
        runtime_signals: [],
        created_at: now,
        updated_at: now,
      };
      await writeFile(join(specsDir, `${missionId}.json`), JSON.stringify(spec, null, 2));

      // Record evidence for 2 of the 3 criteria (crit-001 and crit-002).
      // The evidence command requires --criterion when the task has a Spec.
      // Since the task store now has missionId, we pass --criterion.
      const ev1 = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--command", "echo test-a",
          "--exit", "0",
          "--criterion", "crit-001",
          "--json",
        ],
        dir,
      );
      expect(ev1.exitCode).toBe(0);

      const ev2 = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--command", "echo test-b",
          "--exit", "0",
          "--criterion", "crit-002",
          "--json",
        ],
        dir,
      );
      expect(ev2.exitCode).toBe(0);

      // crit-003 intentionally has no evidence.

      // Run task proof.
      const proofResult = await runCompiled(
        ["task", "proof", "--task", taskId, "--json"],
        dir,
      );

      expect(proofResult.exitCode).toBe(0);
      const proofMap = expectJson<{
        taskId: string;
        missionId: string;
        entries: Array<{
          criterionId: string;
          criterionText: string;
          covered: boolean;
          evidence: Array<{ id: string }>;
        }>;
        uncoveredCount: number;
      }>(proofResult);

      expect(proofMap.taskId).toBe(taskId);
      expect(proofMap.missionId).toBe(missionId);
      expect(proofMap.entries.length).toBe(3);
      expect(proofMap.uncoveredCount).toBe(1);

      const c1 = proofMap.entries.find((e) => e.criterionId === "crit-001");
      const c2 = proofMap.entries.find((e) => e.criterionId === "crit-002");
      const c3 = proofMap.entries.find((e) => e.criterionId === "crit-003");
      expect(c1?.covered).toBe(true);
      expect(c2?.covered).toBe(true);
      expect(c3?.covered).toBe(false);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it.skip("BLOCK: cost-budget exhausted → exit 3, decision BLOCK (deferred to L4.4)", () => {});
});
