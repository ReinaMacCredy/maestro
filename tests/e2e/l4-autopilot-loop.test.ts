/**
 * L4.E2E — Full L4 autopilot loop against ./dist/maestro.
 *
 * Covers:
 *   Plan-check pass  — clean plan → exit 0, plan-check Evidence row written
 *   Plan-check flag  — scope-widens + risk-class-too-low at error severity
 *   PASS             — clean diff + medium risk + ai-review (no errors) → exit 0
 *   HUMAN risk-raise — security ai-review error → effectiveRiskClass critical → exit 2
 *   Threat-model gate — critical security path + no threat-model → threat-model-required reason;
 *                       recording threat-model removes the reason
 *   BLOCK (Rule 11)  — 2 FAIL retries exhaust maxRetries:2 → 3rd verdict request exit 3
 *   task budget verb — exhausted task shows retryCount:2, maxRetries:2, exhausted:true
 *   Autopilot MC     — mission-control --preview autopilot with a mission → exit 0, non-empty output
 *
 * Per ROADMAP.md L4.E2E.
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

// ─── Build once ───────────────────────────────────────────────────────────────

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

// ─── Teardown tracking ────────────────────────────────────────────────────────

const tempDirs: string[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
});

// ─── Shared helpers ───────────────────────────────────────────────────────────

/**
 * Bootstrap a clean temp git repo with `maestro init`.
 */
async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l4-e2e-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);

  const initResult = await runCompiled(["init"], dir);
  if (initResult.exitCode !== 0) {
    throw new Error(`maestro init failed: ${initResult.stderr || initResult.stdout}`);
  }

  // Initial commit so HEAD is valid for diff resolution.
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
    throw new Error(`Unexpected task id: "${taskId}"`);
  }
  return taskId;
}

/**
 * Write a minimal locked v1.json contract directly to the filesystem.
 * Mirrors the approach used in l3-verdict-flow.test.ts.
 */
async function seedContract(
  dir: string,
  taskId: string,
  opts: {
    filesExpected: string[];
    riskClass: string;
    maxAmendments?: number;
    costBudget?: { maxRetries: number; maxWallClockSeconds: number };
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
    intent: "L4 e2e test",
    scope: {
      filesExpected: opts.filesExpected,
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "l4-e2e-test",
    lockedBy: "l4-e2e-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: opts.riskClass,
    amendmentBudget: {
      maxAmendments: opts.maxAmendments ?? 4,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**"],
    },
    ...(opts.costBudget !== undefined ? { costBudget: opts.costBudget } : {}),
  };

  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

/**
 * Write an autopilot policy permitting auto-merge for low and medium risk.
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

/**
 * Write a sensitive-paths policy matching the given globs.
 */
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
 * Commit a file so it appears in the git diff relative to the previous HEAD.
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

// ─── Plan-check scenarios ─────────────────────────────────────────────────────

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("L4 autopilot loop (compiled binary)", () => {
  it(
    "plan-check: pass scenario records evidence; flag scenario emits scope-widens + risk-class-too-low at error",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "plan-check scenarios");
      await seedContract(dir, taskId, {
        filesExpected: ["src/foo.ts"],
        riskClass: "medium",
      });

      // ── Plan-check PASS ──────────────────────────────────────────────────────
      // Plan with intendedFiles within scope and riskClass matching contract.
      const passPlanPath = join(dir, "plan-pass.json");
      await writeFile(
        passPlanPath,
        JSON.stringify({
          intendedFiles: ["src/foo.ts"],
          proofSet: [],
          riskClass: "medium",
          notes: "L4 plan-check pass test",
        }),
      );

      const passResult = await runCompiled(
        ["plan", "check", "--task", taskId, "--plan-file", passPlanPath, "--json"],
        dir,
      );
      // plan check always exits 0 regardless of findings.
      expect(passResult.exitCode).toBe(0);
      const passCheck = expectJson<{
        findings: Array<{ check: string; severity: string }>;
        errorCount: number;
        warnCount: number;
      }>(passResult);
      // No error-severity findings for a clean plan.
      expect(passCheck.errorCount).toBe(0);

      // Verify an evidence row of kind plan-check was written.
      const evidenceListResult = await runCompiled(
        ["evidence", "list", "--task", taskId, "--kind", "plan-check", "--json"],
        dir,
      );
      expect(evidenceListResult.exitCode).toBe(0);
      const evidenceRows = expectJson<Array<{ kind: string }>>(evidenceListResult);
      expect(evidenceRows.length).toBeGreaterThanOrEqual(1);
      expect(evidenceRows.every((r) => r.kind === "plan-check")).toBe(true);

      // ── Plan-check FLAG ──────────────────────────────────────────────────────
      // Plan widens scope (adds src/auth/secret.ts not in contract) and proposes
      // riskClass lower than derived (low vs. medium from contract).
      const flagPlanPath = join(dir, "plan-flag.json");
      await writeFile(
        flagPlanPath,
        JSON.stringify({
          intendedFiles: ["src/foo.ts", "src/auth/secret.ts"],
          proofSet: [],
          riskClass: "low",
          notes: "L4 plan-check flag test",
        }),
      );

      const flagResult = await runCompiled(
        ["plan", "check", "--task", taskId, "--plan-file", flagPlanPath, "--json"],
        dir,
      );
      // Still exits 0 — plan check never blocks.
      expect(flagResult.exitCode).toBe(0);
      const flagCheck = expectJson<{
        findings: Array<{ check: string; severity: string }>;
        errorCount: number;
      }>(flagResult);
      // Should have error-severity findings for both violations.
      expect(flagCheck.errorCount).toBeGreaterThan(0);
      const checks = flagCheck.findings.map((f) => f.check);
      expect(checks).toContain("scope-widens");
      expect(checks).toContain("risk-class-too-low");
      // Both findings at error severity.
      const errFindings = flagCheck.findings.filter((f) => f.severity === "error");
      expect(errFindings.some((f) => f.check === "scope-widens")).toBe(true);
      expect(errFindings.some((f) => f.check === "risk-class-too-low")).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ─── PASS + HUMAN risk-raise scenarios ────────────────────────────────────

  it(
    "PASS: clean diff + bug ai-review (no errors) → exit 0 PASS; HUMAN: security ai-review error → effectiveRiskClass critical → exit 2",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);
      // No sensitive-paths config so src/foo.ts does not hit sensitive-security signal.

      const taskId = await createTask(dir, "PASS + HUMAN risk-raise scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/foo.ts"],
        riskClass: "medium",
      });

      // Commit a file in scope; verdict diff base will be HEAD~1.
      await commitFile(dir, "src/foo.ts", "export const x = 1;\n");

      // Record a clean bug ai-review (no error findings).
      const bugReviewResult = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--kind", "ai-review",
          "--reviewer", "bug",
          "--findings", "[]",
          "--confidence", "0.9",
          "--json",
        ],
        dir,
      );
      expect(bugReviewResult.exitCode).toBe(0);

      // ── PASS ─────────────────────────────────────────────────────────────────
      const passResult = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      expect(passResult.exitCode).toBe(0);
      const passVerdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(passResult);
      expect(passVerdict.decision).toBe("PASS");
      expect(passVerdict.effectiveRiskClass).toBe("medium");

      // ── HUMAN: security review with error severity raises to critical ─────────
      // Add a security ai-review with one error finding.
      const secReviewResult = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--kind", "ai-review",
          "--reviewer", "security",
          "--findings", '[{"severity":"error","message":"hardcoded token"}]',
          "--confidence", "0.9",
          "--json",
        ],
        dir,
      );
      expect(secReviewResult.exitCode).toBe(0);

      // Request verdict again — the security error raises effective class to critical.
      const humanResult = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      expect(humanResult.exitCode).toBe(2);
      const humanVerdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(humanResult);
      expect(humanVerdict.decision).toBe("HUMAN");
      expect(humanVerdict.effectiveRiskClass).toBe("critical");
      const riskReason = humanVerdict.reasons.find((r) => r.category === "risk");
      expect(riskReason).toBeDefined();
      expect(riskReason!.code).toBe("effective-risk-critical");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ─── Threat-model gate ─────────────────────────────────────────────────────

  it(
    "threat-model gate: critical security path without threat-model emits threat-model-required reason; recording threat-model removes it",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);
      // Mark src/auth/** as a sensitive security path so diffs there derive critical.
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "threat-model gate scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/auth/**"],
        riskClass: "low", // agent proposes low; derived will be critical → Rule 1 raises it
      });

      // Touch a security-relevant path.
      await commitFile(dir, "src/auth/x.ts", "export function auth() {}\n");

      // ── First verdict request: no threat-model evidence ────────────────────
      const noTmResult = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      // effectiveRiskClass = critical → HUMAN (rule 4); exit 2.
      expect(noTmResult.exitCode).toBe(2);
      const noTmVerdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(noTmResult);
      expect(noTmVerdict.decision).toBe("HUMAN");
      expect(noTmVerdict.effectiveRiskClass).toBe("critical");

      // The threat-model-required reason must be present.
      const tmRequired = noTmVerdict.reasons.find((r) => r.code === "threat-model-required");
      expect(tmRequired).toBeDefined();
      expect(tmRequired!.category).toBe("policy");

      // ── Record a threat-model evidence row ─────────────────────────────────
      // Use the existing minimal fixture.
      const tmFixturePath = join(__dirname, "..", "fixtures", "threat-models", "minimal.json");
      const tmRecordResult = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--kind", "threat-model",
          "--threat-model-file", tmFixturePath,
          "--json",
        ],
        dir,
      );
      expect(tmRecordResult.exitCode).toBe(0);

      // ── Second verdict request: threat-model-required reason must be gone ──
      const withTmResult = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      // Decision may still be HUMAN (critical → always HUMAN per rule 4), but
      // the threat-model-required reason must no longer appear.
      const withTmVerdict = expectJson<{
        decision: string;
        reasons: Array<{ category: string; code: string }>;
      }>(withTmResult);
      const tmRequiredAfter = withTmVerdict.reasons.find((r) => r.code === "threat-model-required");
      expect(tmRequiredAfter).toBeUndefined();
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ─── BLOCK + task budget ───────────────────────────────────────────────────

  it(
    "BLOCK (Rule 11): 2 FAIL retries exhaust maxRetries:2 → 3rd verdict request exits 3 BLOCK; task budget reflects exhaustion",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "BLOCK cost-budget scenario");
      // Contract only allows src/foo.ts. We will commit src/forbidden.ts which
      // is outside scope → Trust Verifier raises scope error → FAIL.
      await seedContract(dir, taskId, {
        filesExpected: ["src/foo.ts"],
        riskClass: "medium",
        costBudget: { maxRetries: 2, maxWallClockSeconds: 3600 },
      });

      // Commit an allowed file first, then commit a forbidden file.
      // Each verdict request uses HEAD~1 as base so the diff only contains
      // the forbidden file touch.
      await commitFile(dir, "src/foo.ts", "export const x = 1;\n");
      await commitFile(dir, "src/forbidden.ts", "export const y = 2;\n");

      // ── 1st verdict request: FAIL (scope error; retryCount → 1) ─────────────
      const r1 = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      expect(r1.exitCode).toBe(1);
      const v1 = expectJson<{ decision: string }>(r1);
      expect(v1.decision).toBe("FAIL");

      // ── 2nd verdict request: FAIL (retryCount → 2) ──────────────────────────
      const r2 = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      expect(r2.exitCode).toBe(1);
      const v2 = expectJson<{ decision: string }>(r2);
      expect(v2.decision).toBe("FAIL");

      // ── 3rd verdict request: BLOCK (retryCount 2 >= maxRetries 2) ───────────
      const r3 = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      expect(r3.exitCode).toBe(3);
      const v3 = expectJson<{
        decision: string;
        reasons: Array<{ category: string; code: string }>;
      }>(r3);
      expect(v3.decision).toBe("BLOCK");
      const blockReason = v3.reasons.find((r) => r.code === "cost-budget-exhausted");
      expect(blockReason).toBeDefined();
      expect(blockReason!.category).toBe("cost-budget");

      // ── task budget verb ─────────────────────────────────────────────────────
      const budgetResult = await runCompiled(
        ["task", "budget", "--task", taskId, "--json"],
        dir,
      );
      expect(budgetResult.exitCode).toBe(0);
      const budget = expectJson<{
        taskId: string;
        retryCount: number;
        maxRetries: number;
        exhausted: boolean;
      }>(budgetResult);
      expect(budget.taskId).toBe(taskId);
      expect(budget.retryCount).toBe(2);
      expect(budget.maxRetries).toBe(2);
      expect(budget.exhausted).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ─── Autopilot Mission Control screen (mission-create removed in PR-C) ─────
  // The v1 `mission create` verb was removed in PR-C; this test case is a
  // no-op until the v2 equivalent is wired in Phase 4.

  it.skip(
    "autopilot Mission Control screen renders non-empty output with a mission context",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Create a minimal mission so the snapshot has mode === "mission".
      const planPath = join(dir, "plan.json");
      await writeFile(
        planPath,
        JSON.stringify({
          title: "L4 autopilot screen test mission",
          description: "Tests autopilot TUI screen",
          milestones: [
            { id: "m1", title: "Foundation", description: "Core work", order: 0 },
          ],
          features: [
            {
              id: "f1",
              milestoneId: "m1",
              title: "Core task",
              description: "L4 autopilot screen task",
              agentType: "test",
              verificationSteps: ["Verify the autopilot screen renders"],
              fulfills: [],
            },
          ],
        }),
      );

      const missionCreateResult = await runCompiled(
        ["mission", "create", "--file", planPath, "--json"],
        dir,
      );
      expect(missionCreateResult.exitCode).toBe(0);
      const missionData = expectJson<{ mission: { id: string } }>(missionCreateResult);
      const missionId = missionData.mission.id;
      expect(typeof missionId).toBe("string");
      expect(missionId.length).toBeGreaterThan(0);

      // Create a task linked to this mission so the autopilot screen has data.
      const taskId = await createTask(dir, "autopilot screen task");
      // Link the task to the mission by patching tasks.jsonl.
      const { readFile } = await import("node:fs/promises");
      const tasksFile = join(dir, ".maestro", "tasks", "tasks.jsonl");
      let tasksContent = "";
      try {
        tasksContent = await readFile(tasksFile, "utf8");
      } catch {
        // file may not exist
      }
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

      // Render the autopilot screen.
      const renderResult = await runCompiled(
        [
          "mission-control",
          "--mission", missionId,
          "--preview", "autopilot",
          "--format", "plain",
          "--size", "120x40",
        ],
        dir,
      );
      expect(renderResult.exitCode).toBe(0);
      // Output must be non-empty (autopilot screen rendered something).
      const combined = (renderResult.stdout + renderResult.stderr).trim();
      expect(combined.length).toBeGreaterThan(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
