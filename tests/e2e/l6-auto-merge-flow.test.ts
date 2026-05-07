/**
 * L6.E2E — Compiled-binary L6 auto-merge flow end-to-end.
 *
 * Covers 8 scenarios:
 *   S1 Eligible auto-merge — low-risk, all evidence at witnessed-by-ci,
 *      rollback-exercised present; exits 0; fake-gh records one pr merge --auto.
 *   S2 Ineligible: risk class — medium with auto-merge-class-disabled policy;
 *      exits 1; reasons include auto-merge-class-disabled; no merge call.
 *   S3 Ineligible: weak evidence — gating evidence at agent-claimed-locally;
 *      exits 1; reasons include evidence-witness-too-weak; no merge call.
 *   S4 Ineligible: rollback-not-witnessed — no rollback-exercised at
 *      witnessed-by-ci; exits 1; reasons include rollback-not-witnessed.
 *   S5 PR self-weakening attempt (Rule 12) — base owners.yaml does not list
 *      user as sensitive_waiver; verdict override exits 1 with not-authorized.
 *   S6 Override authorized — user in sensitive_waiver at base; override writes
 *      Evidence; subsequent ci verify PR check summary contains "Verdict
 *      overridden by"; conclusion unchanged.
 *   S7 Override rejected — user not in sensitive_waiver; exits 1; no
 *      Evidence row written.
 *   S8 Review-ack consumer — HUMAN verdict at medium; merge auto reports
 *      review-ack-missing; after review ack, code is absent.
 *
 * Extends fake-gh shim to record `pr merge --auto` calls (added at L5.E2E).
 * Each ineligibility scenario asserts the itemised reason list and confirms
 * triggerAutoMerge was NOT invoked.
 *
 * Per ROADMAP.md L6.E2E (trimmed).
 */
import os from "node:os";
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile, readdir } from "node:fs/promises";
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
import { createFakeGhShim } from "../helpers/fake-gh-shim.js";
import type { FakeGhShim } from "../helpers/fake-gh-shim.js";

// ─── Build once ───────────────────────────────────────────────────────────────

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

// ─── Teardown tracking ────────────────────────────────────────────────────────

const tempDirs: string[] = [];
const shims: FakeGhShim[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
  for (const s of shims.splice(0)) {
    await s.cleanup();
  }
});

// ─── Shared helpers ───────────────────────────────────────────────────────────

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l6-e2e-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);

  const initResult = await runCompiled(["init"], dir);
  if (initResult.exitCode !== 0) {
    throw new Error(`maestro init failed: ${initResult.stderr || initResult.stdout}`);
  }

  await runCommand(
    ["git", "commit", "--allow-empty", "-m", "init", "--author", "Test <test@example.com>"],
    dir,
  );

  return dir;
}

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

async function seedContract(
  dir: string,
  taskId: string,
  opts: { filesExpected: string[]; riskClass: string },
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
    intent: "L6 e2e test",
    scope: {
      filesExpected: opts.filesExpected,
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "l6-e2e-test",
    lockedBy: "l6-e2e-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: opts.riskClass,
    amendmentBudget: {
      maxAmendments: 4,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: ["**/secrets/**"],
    },
  };

  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

/** Write an autopilot policy that allows auto-merge for low risk only. */
async function writeLowOnlyAutopilot(dir: string): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  await writeFile(
    join(policyDir, "autopilot.yaml"),
    [
      "kind: autopilot",
      "id: autopilot-policy-l6-low",
      'version: "1"',
      "auto_merge_allowed:",
      "  low: true",
      "  medium: false",
      "  high: false",
      "  critical: false",
      "required_witness_level:",
      "  low: agent-claimed-locally",
      "  medium: witnessed-by-ci",
      "  high: witnessed-by-maestro",
      "  critical: witnessed-by-maestro",
    ].join("\n"),
  );
}

/** Write an autopilot policy that allows auto-merge for low and medium. */
async function writeMediumPermissiveAutopilot(dir: string): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  await writeFile(
    join(policyDir, "autopilot.yaml"),
    [
      "kind: autopilot",
      "id: autopilot-policy-l6-medium",
      'version: "1"',
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

async function headCommitSha(dir: string): Promise<string> {
  const r = await runCommand(["git", "rev-parse", "HEAD"], dir);
  if (r.exitCode !== 0) throw new Error(`git rev-parse HEAD failed: ${r.stderr}`);
  return r.stdout.trim();
}

async function writeEventFile(dir: string, pr: number): Promise<string> {
  const eventFile = join(dir, "github-event.json");
  await writeFile(eventFile, JSON.stringify({ pull_request: { number: pr } }));
  return eventFile;
}

async function buildCiEnv(
  dir: string,
  opts: {
    repo: string;
    pr: number;
    shimBinDir: string;
    githubOutputFile: string;
    eventFile: string;
  },
): Promise<Record<string, string>> {
  const sha = await headCommitSha(dir);
  return {
    GITHUB_ACTIONS: "true",
    GITHUB_REPOSITORY: opts.repo,
    GITHUB_REF: `refs/pull/${opts.pr}/merge`,
    GITHUB_BASE_REF: "main",
    GITHUB_SHA: sha,
    GITHUB_EVENT_PATH: opts.eventFile,
    GITHUB_OUTPUT: opts.githubOutputFile,
    GITHUB_TOKEN: "fake",
    PATH: `${opts.shimBinDir}:${process.env.PATH ?? ""}`,
  };
}

/**
 * Write a rollback-exercised evidence row directly to the evidence store.
 * The CLI does not expose this kind; it is seeded by writing the JSON file.
 */
async function seedRollbackEvidence(
  dir: string,
  taskId: string,
  witnessLevel: "witnessed-by-ci" | "witnessed-by-maestro" | "agent-claimed-locally",
): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });

  // Generate a stable evidence ID (deterministic for testing).
  // Suffix must match /^[0-9a-f]{6}$/ — only hex characters.
  const ts = String(Date.now()).padStart(13, "0");
  const id = `evd-${ts}-ab0001`;

  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "rollback-exercised",
    witness_level: witnessLevel,
    created_at: new Date().toISOString(),
    payload: {
      command: "kubectl rollout undo deployment/app",
      exit: 0,
    },
  };

  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

/**
 * Seed a command evidence row at the given witness level.
 * Used to simulate gating evidence already recorded.
 */
async function seedCommandEvidence(
  dir: string,
  taskId: string,
  witnessLevel: "witnessed-by-ci" | "witnessed-by-maestro" | "agent-claimed-locally" | "agent-claimed-and-not-reproducible",
  suffix = "000001",
): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });

  const ts = String(Date.now()).padStart(13, "0");
  const id = `evd-${ts}-${suffix}`;

  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "command",
    witness_level: witnessLevel,
    created_at: new Date().toISOString(),
    payload: {
      command: "bun test",
      exit: 0,
    },
  };

  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

/**
 * Seed a deploy-readiness Evidence row. Brings L7 into scope so the
 * rollback-not-witnessed predicate applies even without a Spec rollout_plan.
 */
async function seedDeployReadinessEvidence(dir: string, taskId: string): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });
  const ts = String(Date.now()).padStart(13, "0");
  const id = `evd-${ts}-d70001`;
  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "deploy-readiness",
    witness_level: "witnessed-by-ci",
    created_at: new Date().toISOString(),
    payload: {
      task_id: taskId,
      checks: {
        feature_flag: { ok: false },
        canary_plan: { ok: false },
        rollback: { ok: false },
        owner: { ok: false },
      },
      gate: "fail",
    },
  };
  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

/**
 * Request a verdict for the given task, returning the verdict JSON.
 * Uses HEAD~1 as base so a single committed file produces a clean diff.
 */
async function requestVerdict(dir: string, taskId: string): Promise<{ id: string; decision: string; effectiveRiskClass: string }> {
  const result = await runCompiled(
    ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
    dir,
  );
  if (result.exitCode !== 0 && result.exitCode !== 1 && result.exitCode !== 2 && result.exitCode !== 3) {
    throw new Error(`verdict request failed unexpectedly: ${result.stderr || result.stdout}`);
  }
  return expectJson<{ id: string; decision: string; effectiveRiskClass: string }>(result);
}

// ─── Scenarios ────────────────────────────────────────────────────────────────

describe("L6 auto-merge flow (compiled binary)", () => {
  // ── S1: Eligible auto-merge (happy path) ──────────────────────────────────

  it(
    "S1 Eligible: low-risk, ci-witnessed evidence + rollback; exits 0; pr merge --auto recorded once",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      // medium-permissive: auto_merge_allowed.medium=true; diff of src/feature.ts
      // yields effective risk class medium (deriveRiskClassFromDiff raises low→medium
      // for src/ paths per the default risk policy).
      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 eligible auto-merge");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Seed all-green gating evidence at witnessed-by-ci
      await seedCommandEvidence(dir, taskId, "witnessed-by-ci", "ce0001");
      // Seed rollback-exercised at witnessed-by-ci
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      // Commit the allowed file so diff is clean
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Request a verdict (should PASS: medium risk + auto_merge_allowed.medium=true)
      const verdict = await requestVerdict(dir, taskId);
      expect(verdict.decision).toBe("PASS");
      expect(verdict.effectiveRiskClass).toBe("medium");

      // Run merge auto — should be eligible and trigger
      const result = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result.exitCode).toBe(0);
      const output = expectJson<{ eligible: boolean; reasons: unknown[]; merged: boolean }>(result);
      expect(output.eligible).toBe(true);
      expect(output.merged).toBe(true);

      // Fake-gh shim: exactly one pr merge --auto call
      const state = shim.readState();
      expect(state.prMergeCalls.length).toBe(1);
      expect(state.prMergeCalls[0]!.pr).toBe(1);
      expect(state.prMergeCalls[0]!.repo).toBe("fixture/repo");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S2: Ineligible — risk class disabled ────────────────────────────────────

  it(
    "S2 Ineligible: medium risk, auto-merge-class-disabled policy; exits 1; no merge call",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      // Low-only policy: medium auto-merge disabled (auto_merge_allowed.medium=false).
      // With a medium-risk diff, computeRisk issues HUMAN (auto-merge-not-allowed).
      // merge auto then sees verdict-not-pass AND auto-merge-class-disabled.
      await writeLowOnlyAutopilot(dir);

      const taskId = await createTask(dir, "L6 medium class disabled");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await seedCommandEvidence(dir, taskId, "witnessed-by-ci", "ce0001");
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // medium + auto_merge_allowed.medium=false → verdict is HUMAN
      const verdict = await requestVerdict(dir, taskId);
      expect(verdict.decision).toBe("HUMAN");
      expect(verdict.effectiveRiskClass).toBe("medium");

      const result = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{ eligible: boolean; reasons: Array<{ code: string }> }>(result);
      expect(output.eligible).toBe(false);
      expect(output.reasons.some((r) => r.code === "verdict-not-pass")).toBe(true);
      expect(output.reasons.some((r) => r.code === "auto-merge-class-disabled")).toBe(true);

      // No merge call
      const state = shim.readState();
      expect(state.prMergeCalls.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S3: Ineligible — weak evidence ──────────────────────────────────────────

  it(
    "S3 Ineligible: gating evidence at agent-claimed-locally; exits 1; evidence-witness-too-weak; no merge call",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      // Permissive policy so verdict is PASS; autoMergeEligible then catches
      // the weak evidence independently.
      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 weak evidence");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Gating evidence at weak level
      await seedCommandEvidence(dir, taskId, "agent-claimed-locally", "ce0001");
      // Rollback present at witnessed-by-ci so rollback-not-witnessed is not triggered
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const verdict = await requestVerdict(dir, taskId);
      expect(verdict.decision).toBe("PASS");

      const result = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{ eligible: boolean; reasons: Array<{ code: string }> }>(result);
      expect(output.eligible).toBe(false);
      expect(output.reasons.some((r) => r.code === "evidence-witness-too-weak")).toBe(true);

      const state = shim.readState();
      expect(state.prMergeCalls.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S4: Ineligible — rollback-not-witnessed ──────────────────────────────────

  it(
    "S4 Ineligible: no rollback-exercised at witnessed-by-ci; exits 1; rollback-not-witnessed; no merge call",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      // Permissive policy so verdict is PASS; autoMergeEligible catches the
      // missing rollback evidence independently.
      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 rollback not witnessed");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Gating evidence strong, but NO rollback evidence.
      await seedCommandEvidence(dir, taskId, "witnessed-by-ci", "ce0001");
      // Seed a deploy-readiness Evidence row so the rollback predicate
      // applies (L7 is in scope for this task). Without it, the rollback
      // predicate is skipped — by design — and merge would proceed.
      await seedDeployReadinessEvidence(dir, taskId);
      // (intentionally omit seedRollbackEvidence)

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const verdict = await requestVerdict(dir, taskId);
      expect(verdict.decision).toBe("PASS");

      const result = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{ eligible: boolean; reasons: Array<{ code: string }> }>(result);
      expect(output.eligible).toBe(false);
      expect(output.reasons.some((r) => r.code === "rollback-not-witnessed")).toBe(true);

      const state = shim.readState();
      expect(state.prMergeCalls.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S5: PR self-weakening attempt (Rule 12) ──────────────────────────────────

  it(
    "S5 Rule 12: base owners.yaml does not list user; verdict override rejected with not-authorized",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 rule 12 self-weakening");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await seedCommandEvidence(dir, taskId, "witnessed-by-ci", "ce0001");
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Request a verdict so there's something to override
      const verdict = await requestVerdict(dir, taskId);

      // Commit owners.yaml WITHOUT the current user in sensitive_waiver.
      // This simulates the base branch state (Rule 12: loaded from base, not head).
      const policyDir = join(dir, ".maestro", "policies");
      await writeFile(
        join(policyDir, "owners.yaml"),
        [
          "policy_approver:",
          "  - admin",
          "ratchet_approver:",
          "  - admin",
          "sensitive_waiver:",
          "  - other-user-not-the-runner",
        ].join("\n"),
      );
      await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
      await runCommand(
        ["git", "commit", "-m", "chore: add owners.yaml without self", "--author", "Test <test@example.com>"],
        dir,
      );

      // Attempt override: --base HEAD so it reads the just-committed owners.yaml.
      // The running user is NOT in sensitive_waiver → rejected.
      const result = await runCompiled(
        ["verdict", "override", "--task", taskId, "--pr", "1",
          "--reason", "emergency hotfix self-promo attempt",
          "--verdict", verdict.id,
          "--base", "HEAD"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      // The error message must include "not-authorized"
      expect(result.stderr + result.stdout).toContain("not-authorized");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S6: Override authorized ──────────────────────────────────────────────────

  it(
    "S6 Override authorized: user in sensitive_waiver; evidence written; ci verify posts PASS check with unchanged conclusion",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 override authorized");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Commit owners.yaml WITH the current user in sensitive_waiver FIRST,
      // so that --base HEAD~1 points to this commit when overriding.
      const currentUser = os.userInfo().username;
      const policyDir = join(dir, ".maestro", "policies");
      await writeFile(
        join(policyDir, "owners.yaml"),
        [
          "policy_approver:",
          "  - admin",
          "ratchet_approver:",
          "  - admin",
          "sensitive_waiver:",
          `  - ${currentUser}`,
        ].join("\n"),
      );
      await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
      await runCommand(
        ["git", "commit", "-m", "chore: owners.yaml with current user", "--author", "Test <test@example.com>"],
        dir,
      );

      // Commit the feature file SECOND — this is what ci verify's diff should cover.
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Run ci verify using HEAD~1 as base (diff = feature file only).
      const githubOutputFile = join(dir, "github-output.txt");
      const eventFile = await writeEventFile(dir, 1);
      const ciEnv = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile,
        eventFile,
      });

      const ciResult = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv },
      );
      expect(ciResult.exitCode).toBe(0);
      const ciVerdict = expectJson<{ id: string; decision: string }>(ciResult);
      expect(ciVerdict.decision).toBe("PASS");

      // Record override with --base HEAD~1 (= owners.yaml commit).
      // Override references the verdict from the ci verify run above.
      const overrideResult = await runCompiled(
        ["verdict", "override", "--task", taskId, "--pr", "1",
          "--reason", "Authorized override for L6 e2e test",
          "--verdict", ciVerdict.id,
          "--base", "HEAD~1",
          "--json"],
        dir,
      );

      expect(overrideResult.exitCode).toBe(0);
      const overrideRow = expectJson<{ id: string; kind: string }>(overrideResult);
      expect(overrideRow.kind).toBe("verdict-override");

      // Verify evidence list shows the verdict-override row
      const listResult = await runCompiled(
        ["evidence", "list", "--task", taskId, "--json"],
        dir,
      );
      expect(listResult.exitCode).toBe(0);
      const rows = expectJson<Array<{ kind: string }>>(listResult);
      expect(rows.some((r) => r.kind === "verdict-override")).toBe(true);

      // Run ci verify AGAIN — this time the override Evidence row exists, so
      // run-ci-verify should look it up by task and pass it to postPrCheck for
      // summary rendering. Conclusion stays mapped from verdict (success here).
      const ciResult2 = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv },
      );
      expect(ciResult2.exitCode).toBe(0);

      const state = shim.readState();
      expect(state.checkRuns.length).toBeGreaterThanOrEqual(2);
      const lastRun = state.checkRuns[state.checkRuns.length - 1]!;
      expect(lastRun.conclusion).toBe("success");
      const summary = lastRun.output?.summary ?? "";
      expect(summary).toContain("Verdict overridden by");
      expect(summary).toContain(currentUser);
      expect(summary).toContain("Authorized override for L6 e2e test");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S7: Override rejected ────────────────────────────────────────────────────

  it(
    "S7 Override rejected: user not in sensitive_waiver; exits 1; no Evidence row written",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L6 override rejected");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const verdict = await requestVerdict(dir, taskId);

      // Commit owners.yaml WITHOUT the current user
      const policyDir = join(dir, ".maestro", "policies");
      await writeFile(
        join(policyDir, "owners.yaml"),
        [
          "policy_approver:",
          "  - admin",
          "ratchet_approver:",
          "  - admin",
          "sensitive_waiver:",
          "  - someone-else",
        ].join("\n"),
      );
      await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
      await runCommand(
        ["git", "commit", "-m", "chore: owners.yaml no self", "--author", "Test <test@example.com>"],
        dir,
      );

      const countBefore = await countEvidenceRows(dir, taskId);

      const result = await runCompiled(
        ["verdict", "override", "--task", taskId, "--pr", "1",
          "--reason", "Trying to self-approve",
          "--verdict", verdict.id,
          "--base", "HEAD"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      expect(result.stderr + result.stdout).toContain("not-authorized");

      // No new evidence rows must have been written
      const countAfter = await countEvidenceRows(dir, taskId);
      expect(countAfter).toBe(countBefore);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S8: Review-ack consumer ──────────────────────────────────────────────────

  it(
    "S8 Review-ack: HUMAN verdict; merge auto reports review-ack-missing; after review ack the code is absent",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      // Low-only policy: medium triggers HUMAN (auto-merge-not-allowed)
      await writeLowOnlyAutopilot(dir);

      const taskId = await createTask(dir, "L6 review ack consumer");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await seedCommandEvidence(dir, taskId, "witnessed-by-ci", "ce0001");
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // medium + auto_merge_allowed.medium=false → HUMAN verdict
      const verdict = await requestVerdict(dir, taskId);
      expect(verdict.decision).toBe("HUMAN");

      // Before review-ack: merge auto should report review-ack-missing
      const result1 = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result1.exitCode).toBe(1);
      const output1 = expectJson<{ eligible: boolean; reasons: Array<{ code: string }> }>(result1);
      expect(output1.reasons.some((r) => r.code === "review-ack-missing")).toBe(true);

      // Record review-ack for this verdict
      const ackResult = await runCompiled(
        ["review", "ack", "--task", taskId, "--verdict", verdict.id,
          "--criterion", "All acceptance criteria verified"],
        dir,
      );
      expect(ackResult.exitCode).toBe(0);

      // After review-ack: merge auto should NOT report review-ack-missing
      // (other reasons like verdict-not-pass and auto-merge-class-disabled may still apply)
      const result2 = await runCompiled(
        ["merge", "auto", "--pr", "1", "--task", taskId, "--base", "HEAD~1",
          "--repo", "fixture/repo", "--json"],
        dir,
        { env: { PATH: `${shim.binDir}:${process.env.PATH ?? ""}` } },
      );

      expect(result2.exitCode).toBe(1);
      const output2 = expectJson<{ eligible: boolean; reasons: Array<{ code: string }> }>(result2);
      // review-ack-missing must be gone
      expect(output2.reasons.some((r) => r.code === "review-ack-missing")).toBe(false);

      // No merge calls in either run
      const state = shim.readState();
      expect(state.prMergeCalls.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});

// ─── Internal helpers ─────────────────────────────────────────────────────────

async function countEvidenceRows(dir: string, taskId: string): Promise<number> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  try {
    const entries = await readdir(evidenceDir);
    return entries.filter((f) => f.endsWith(".json")).length;
  } catch {
    return 0;
  }
}
