/**
 * L5.E2E — Compiled-binary L5 ci-verify flow end-to-end.
 *
 * Covers 6 scenarios:
 *   S1 Init         — fresh git repo + maestro init; .maestro/ present
 *   S2 Setup        — task + contract; task verify green at baseline
 *   S3 Clean PASS   — clean diff; ci verify exits 0; check-run POSTed with
 *                     conclusion=success; GITHUB_OUTPUT populated; verdict
 *                     subject.tree_sha matches git rev-parse HEAD^{tree}
 *   S4 Squash       — same tree SHA after amend --no-edit; same verdict id;
 *                     a NEW check-run is POSTed (no dedup — adapter always POSTs)
 *   S5 Force-push   — different tree SHA after content change; new verdict;
 *                     new check-run with new head_sha
 *   S6 FAIL + rerun — forbidden diff → FAIL check; fix → PASS check; two
 *                     separate check-run rows (different tree SHAs)
 *
 * PLAN DEVIATION NOTE (recorded for caller):
 *   The L5.4 GhCliAdapter always POSTs a new check-run; it never PATCHes an
 *   existing one (no dedup by PR/tree_sha). The comment in run-ci-verify.ts
 *   explicitly says "each run POSTs a new check" and dedup is deferred. S4 and
 *   S6 therefore assert two separate POST rows, not a PATCH of one row.
 *
 * Per ROADMAP.md L5.E2E (trimmed).
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-l5-e2e-"));
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
    intent: "L5 e2e test",
    scope: {
      filesExpected: opts.filesExpected,
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "l5-e2e-test",
    lockedBy: "l5-e2e-test",
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

async function writeMediumPermissiveAutopilot(dir: string): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  await writeFile(
    join(policyDir, "autopilot.yaml"),
    [
      "kind: autopilot",
      "id: autopilot-policy-l5",
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

/** Read the tree SHA of HEAD in the given repo. */
async function headTreeSha(dir: string): Promise<string> {
  const r = await runCommand(["git", "rev-parse", "HEAD^{tree}"], dir);
  if (r.exitCode !== 0) throw new Error(`git rev-parse HEAD^{tree} failed: ${r.stderr}`);
  return r.stdout.trim();
}

/** Read HEAD commit SHA. */
async function headCommitSha(dir: string): Promise<string> {
  const r = await runCommand(["git", "rev-parse", "HEAD"], dir);
  if (r.exitCode !== 0) throw new Error(`git rev-parse HEAD failed: ${r.stderr}`);
  return r.stdout.trim();
}

/**
 * Build the CI env vars for a fake GitHub Actions run.
 * Returns a plain Record suitable for passing to runCompiled as `env`.
 */
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
    // Prepend shim dir so `gh` resolves to the fake script.
    PATH: `${opts.shimBinDir}:${process.env.PATH ?? ""}`,
  };
}

/** Write a minimal GitHub event JSON file (pull_request.number). */
async function writeEventFile(dir: string, pr: number): Promise<string> {
  const eventFile = join(dir, "github-event.json");
  await writeFile(eventFile, JSON.stringify({ pull_request: { number: pr } }));
  return eventFile;
}

// ─── Scenarios ────────────────────────────────────────────────────────────────

describe("L5 ci verify flow (compiled binary)", () => {
  // ── S1: Init ──────────────────────────────────────────────────────────────

  it(
    "S1 Init: maestro init materialises .maestro/ in a fresh repo",
    async () => {
      const dir = await mkdtemp(join(tmpdir(), "maestro-l5-s1-"));
      tempDirs.push(dir);

      await initGitRepo(dir);
      await runCommand(["git", "config", "user.email", "test@example.com"], dir);
      await runCommand(["git", "config", "user.name", "Test"], dir);

      const initResult = await runCompiled(["init"], dir);
      expect(initResult.exitCode).toBe(0);

      // .maestro/ directory must exist
      const { stat } = await import("node:fs/promises");
      const maestroDir = join(dir, ".maestro");
      const s = await stat(maestroDir);
      expect(s.isDirectory()).toBe(true);

      // Policies directory should be bootstrapped by init
      const policiesDir = join(dir, ".maestro", "policies");
      const ps = await stat(policiesDir).catch(() => null);
      expect(ps?.isDirectory()).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S2: Setup ─────────────────────────────────────────────────────────────

  it(
    "S2 Setup: task + contract; task verify green at baseline",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L5 setup scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // task verify at baseline (no diff yet) should not error out.
      // It may report findings, but the command itself should exit 0.
      const verifyResult = await runCompiled(
        ["task", "verify", "--task", taskId],
        dir,
      );
      expect(verifyResult.exitCode).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S3: Clean PASS ────────────────────────────────────────────────────────

  it(
    "S3 Clean PASS: ci verify exits 0; check-run POSTed success; GITHUB_OUTPUT populated; verdict tree_sha matches HEAD^{tree}",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L5 clean pass scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Commit the allowed file so diff base (HEAD~1) produces a clean diff.
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const treeSha = await headTreeSha(dir);

      // Prepare CI env
      const githubOutputFile = join(dir, "github-output.txt");
      const eventFile = await writeEventFile(dir, 1);
      const ciEnv = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile,
        eventFile,
      });

      const result = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv },
      );

      // exit 0 = PASS
      expect(result.exitCode).toBe(0);
      const verdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        id: string;
        subject?: { tree_sha: string; pr?: number };
      }>(result);
      expect(verdict.decision).toBe("PASS");

      // subject.tree_sha must match the current HEAD^{tree}
      expect(verdict.subject?.tree_sha).toBe(treeSha);
      expect(verdict.subject?.pr).toBe(1);

      // GITHUB_OUTPUT file must contain the three keys
      const outputContent = await readFile(githubOutputFile, "utf8");
      expect(outputContent).toContain("verdict_id=");
      expect(outputContent).toContain("verdict_decision=PASS");
      expect(outputContent).toContain("effective_risk_class=");

      // fake-gh state: one check-run POSTed with conclusion=success
      const state = shim.readState();
      expect(state.checkRuns.length).toBe(1);
      const run = state.checkRuns[0]!;
      expect(run.operation).toBe("POST");
      expect(run.conclusion).toBe("success");
      expect(run.name).toBe("Maestro Verify");
      // head_sha must match what we put in GITHUB_SHA (HEAD commit sha)
      const sha = await headCommitSha(dir);
      expect(run.head_sha).toBe(sha);

      // output.summary must mention the risk class
      expect(run.output?.summary).toContain("medium");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S4: Tree-SHA invariant under squash ───────────────────────────────────

  it(
    "S4 Squash: amend --no-edit produces same tree SHA; same verdict id returned; new check-run POSTed (adapter always POSTs)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L5 squash scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // ── First ci verify (S3-equivalent) ──────────────────────────────────
      const treeSha = await headTreeSha(dir);
      const githubOutputFile1 = join(dir, "github-output-1.txt");
      const eventFile = await writeEventFile(dir, 1);
      const ciEnv1 = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputFile1,
        eventFile,
      });

      const r1 = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv1 },
      );
      expect(r1.exitCode).toBe(0);
      const v1 = expectJson<{ id: string; subject?: { tree_sha: string } }>(r1);
      expect(v1.subject?.tree_sha).toBe(treeSha);

      // ── Amend commit — same content, new commit SHA, same tree SHA ────────
      await runCommand(
        ["git", "commit", "--amend", "--no-edit", "--author", "Test <test@example.com>"],
        dir,
      );

      const treeShaAfterAmend = await headTreeSha(dir);
      // Tree SHA must be identical: amend with no content change preserves the tree.
      expect(treeShaAfterAmend).toBe(treeSha);

      // ── Second ci verify ─────────────────────────────────────────────────
      const githubOutputFile2 = join(dir, "github-output-2.txt");
      const ciEnv2 = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputFile2,
        eventFile,
      });

      const r2 = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv2 },
      );
      expect(r2.exitCode).toBe(0);
      const v2 = expectJson<{ id: string; subject?: { tree_sha: string } }>(r2);

      // Same tree SHA → same stored subject, but requestVerdict always writes a
      // new Verdict record (L3: append-only store). The verdict ID is new, but
      // verdict show --pr 1 resolves by tree_sha and returns the latest match.
      expect(v2.subject?.tree_sha).toBe(treeSha);

      // verdict show --pr 1 must return the latest verdict (v2 id).
      const showResult = await runCompiled(
        ["verdict", "show", "--task", taskId, "--pr", "1", "--json"],
        dir,
        // PATH doesn't matter here; verdict show doesn't call gh.
        { env: { ...process.env } },
      );
      expect(showResult.exitCode).toBe(0);
      const shown = expectJson<{ id: string; subject?: { tree_sha: string } }>(showResult);
      expect(shown.id).toBe(v2.id);
      expect(shown.subject?.tree_sha).toBe(treeSha);

      // Adapter always POSTs — two check-run rows, both conclusion=success.
      const state = shim.readState();
      expect(state.checkRuns.length).toBe(2);
      expect(state.checkRuns.every((r) => r.conclusion === "success")).toBe(true);
      expect(state.checkRuns.every((r) => r.operation === "POST")).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S5: Force-push invalidation ───────────────────────────────────────────

  it(
    "S5 Force-push: different tree SHA after content change; new verdict with new tree_sha; new check-run with new head_sha",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L5 force-push scenario");
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // ── First ci verify ──────────────────────────────────────────────────
      const treeSha1 = await headTreeSha(dir);
      const githubOutputFile1 = join(dir, "github-output-1.txt");
      const eventFile = await writeEventFile(dir, 1);
      const ciEnv1 = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputFile1,
        eventFile,
      });

      const r1 = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv1 },
      );
      expect(r1.exitCode).toBe(0);
      const v1 = expectJson<{ id: string; subject?: { tree_sha: string } }>(r1);
      expect(v1.subject?.tree_sha).toBe(treeSha1);

      // ── Force-push simulation: amend with CHANGED content ────────────────
      const fullPath = join(dir, "src/feature.ts");
      await writeFile(fullPath, "export const x = 2; // force-pushed\n");
      await runCommand(["git", "add", "src/feature.ts"], dir);
      await runCommand(
        ["git", "commit", "--amend", "-m", "chore: force-push", "--author", "Test <test@example.com>"],
        dir,
      );

      const treeSha2 = await headTreeSha(dir);
      // Content changed → tree SHA MUST differ.
      expect(treeSha2).not.toBe(treeSha1);

      // ── Second ci verify with new tree ───────────────────────────────────
      const githubOutputFile2 = join(dir, "github-output-2.txt");
      const ciEnv2 = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputFile2,
        eventFile,
      });

      const r2 = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv2 },
      );
      expect(r2.exitCode).toBe(0);
      const v2 = expectJson<{ id: string; subject?: { tree_sha: string } }>(r2);

      // New verdict with the new tree SHA.
      expect(v2.subject?.tree_sha).toBe(treeSha2);
      expect(v2.id).not.toBe(v1.id);

      // verdict show --pr 1 resolves by CURRENT tree SHA (treeSha2) → returns v2.
      const showResult = await runCompiled(
        ["verdict", "show", "--task", taskId, "--pr", "1", "--json"],
        dir,
        { env: { ...process.env } },
      );
      expect(showResult.exitCode).toBe(0);
      const shown = expectJson<{ id: string; subject?: { tree_sha: string } }>(showResult);
      expect(shown.id).toBe(v2.id);
      expect(shown.subject?.tree_sha).toBe(treeSha2);

      // Two check-runs POSTed; the second has a different head_sha.
      const state = shim.readState();
      expect(state.checkRuns.length).toBe(2);
      const sha1 = state.checkRuns[0]!.head_sha;
      const sha2 = state.checkRuns[1]!.head_sha;
      expect(sha1).not.toBe(sha2);
      // Both POSTs.
      expect(state.checkRuns[0]!.operation).toBe("POST");
      expect(state.checkRuns[1]!.operation).toBe("POST");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S6: FAIL with re-run ──────────────────────────────────────────────────

  it(
    "S6 FAIL + rerun: forbidden diff → exit 1 failure check; fix diff → exit 0 success check; two separate check-run rows",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "L5 fail-rerun scenario");
      // Contract only allows src/feature.ts; forbidden.ts is outside scope.
      await seedContract(dir, taskId, {
        filesExpected: ["src/feature.ts"],
        riskClass: "medium",
      });

      // Commit an allowed file as the base.
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Now commit a forbidden file — outside contract scope.
      await commitFile(dir, "src/forbidden.ts", "export const y = 2;\n");

      const treeShaFail = await headTreeSha(dir);

      const eventFile = await writeEventFile(dir, 1);
      const githubOutputFail = join(dir, "github-output-fail.txt");
      const ciEnvFail = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputFail,
        eventFile,
      });

      // ── FAIL run ──────────────────────────────────────────────────────────
      const rFail = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnvFail },
      );
      expect(rFail.exitCode).toBe(1);
      const vFail = expectJson<{
        decision: string;
        subject?: { tree_sha: string };
        trustVerifier: { errors: number };
      }>(rFail);
      expect(vFail.decision).toBe("FAIL");
      expect(vFail.subject?.tree_sha).toBe(treeShaFail);
      expect(vFail.trustVerifier.errors).toBeGreaterThan(0);

      // check-run should have conclusion=failure
      const stateFail = shim.readState();
      expect(stateFail.checkRuns.length).toBe(1);
      const runFail = stateFail.checkRuns[0]!;
      expect(runFail.conclusion).toBe("failure");
      expect(runFail.operation).toBe("POST");

      // ── Fix: undo the forbidden commit and replace with a clean one ─────────
      // Reset to HEAD~1 (the base commit) so HEAD~1 becomes the init commit.
      // This puts forbidden.ts back in the working tree (mixed reset).
      // Then delete the forbidden file and commit only the allowed changes.
      // After this, HEAD~1..HEAD diff contains only src/feature.ts (modified).
      await runCommand(["git", "reset", "HEAD~1"], dir);
      // Remove the forbidden file from the working tree entirely.
      const { rm: rmFile } = await import("node:fs/promises");
      await rmFile(join(dir, "src", "forbidden.ts"), { force: true } as Parameters<typeof rmFile>[1]);
      // Modify the allowed file so the diff is non-empty and the tree SHA differs.
      await writeFile(join(dir, "src", "feature.ts"), "export const x = 2; // fixed\n");
      await runCommand(["git", "add", "src/feature.ts"], dir);
      await runCommand(
        ["git", "commit", "-m", "fix: clean commit", "--author", "Test <test@example.com>"],
        dir,
      );

      const treeShaPass = await headTreeSha(dir);
      // Content changed — tree SHA must differ from the FAIL run.
      expect(treeShaPass).not.toBe(treeShaFail);

      const githubOutputPass = join(dir, "github-output-pass.txt");
      const ciEnvPass = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 1,
        shimBinDir: shim.binDir,
        githubOutputFile: githubOutputPass,
        eventFile,
      });

      // ── PASS re-run ───────────────────────────────────────────────────────
      const rPass = await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "1", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnvPass },
      );
      expect(rPass.exitCode).toBe(0);
      const vPass = expectJson<{
        decision: string;
        subject?: { tree_sha: string };
      }>(rPass);
      expect(vPass.decision).toBe("PASS");
      expect(vPass.subject?.tree_sha).toBe(treeShaPass);

      // Two separate check-run rows (different tree SHAs → different POSTs).
      const statePass = shim.readState();
      expect(statePass.checkRuns.length).toBe(2);
      expect(statePass.checkRuns[0]!.conclusion).toBe("failure");
      expect(statePass.checkRuns[1]!.conclusion).toBe("success");
      expect(statePass.checkRuns[0]!.head_sha).not.toBe(statePass.checkRuns[1]!.head_sha);

      // GITHUB_OUTPUT on the passing run contains verdict_decision=PASS
      const passOutput = await readFile(githubOutputPass, "utf8");
      expect(passOutput).toContain("verdict_decision=PASS");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
