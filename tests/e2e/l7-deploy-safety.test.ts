/**
 * L7.E2E — Compiled-binary L7 deploy-safety flow end-to-end.
 *
 * Covers 12 scenarios:
 *   S1  Spec v1 → v2 read migration — v1 file on disk; spec show returns v2 shape; file unchanged.
 *   S2  Witnessed rollback (local) — deploy rollback exits 0; rollback-exercised Evidence at witnessed-by-maestro.
 *   S3  Witnessed rollback (CI) — same but GITHUB_ACTIONS=true; witness is witnessed-by-ci.
 *   S4  Deploy gate happy path — all 4 checks pass; gate=pass; exit 0.
 *   S5  Deploy gate missing rollback — no rollback Evidence; rollback.ok=false; exit 1.
 *   S6  Deploy gate missing flag — Spec has no feature_flag; feature_flag.ok=false; exit 1.
 *   S7  Deploy gate missing owner — owners.yaml has no deploy_approver; owner.ok=false; exit 1.
 *   S8  Runtime check happy path — Prometheus fixture returns 0.42 (< 1.0 threshold); pass=true.
 *   S9  Runtime check threshold breach — fixture returns 1.5 (> 1.0 threshold); pass=false.
 *   S10 Runtime check unsupported provider — signal has provider=datadog; pass=false note=unsupported provider.
 *   S11 L7.9 deploy authorized — deploy-readiness pass + PR author in deploy_approver; CI check success.
 *   S12 L7.9 deploy not-authorized — same setup but PR author NOT in deploy_approver; CI check failure
 *       with "deploy not authorized" in summary.
 *
 * Per ROADMAP.md L7.E2E (trimmed).
 */
import type { Server } from "bun";
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
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

// ─── Build once ────────────────────────────────────────────────────────────────

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

// ─── Teardown tracking ─────────────────────────────────────────────────────────

const tempDirs: string[] = [];
const shims: FakeGhShim[] = [];
const servers: Server[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
  for (const s of shims.splice(0)) {
    await s.cleanup();
  }
  for (const srv of servers.splice(0)) {
    srv.stop(true);
  }
});

// ─── Shared helpers ────────────────────────────────────────────────────────────

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l7-e2e-"));
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

/**
 * Create a task via CLI (no missionId). Returns the task ID.
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
 * Create a task and link it to a mission by patching the tasks.jsonl.
 * The task store format is one JSON object per line; we replace the task line
 * with one that has missionId set.
 */
async function createTaskWithMission(
  dir: string,
  title: string,
  missionId: string,
): Promise<string> {
  const taskId = await createTask(dir, title);
  const jsonlPath = join(dir, ".maestro", "tasks", "tasks.jsonl");
  const content = await readFile(jsonlPath, "utf8");
  const lines = content.split("\n").filter((l) => l.trim().length > 0);
  const patched = lines.map((line) => {
    const obj = JSON.parse(line) as Record<string, unknown>;
    if (obj["id"] === taskId) {
      return JSON.stringify({ ...obj, missionId });
    }
    return line;
  });
  await writeFile(jsonlPath, patched.join("\n") + "\n");
  return taskId;
}

/**
 * Write a v2 Spec JSON directly to the spec store path.
 */
async function writeSpec(
  dir: string,
  missionId: string,
  spec: Record<string, unknown>,
): Promise<void> {
  const specsDir = join(dir, ".maestro", "specs");
  await mkdir(specsDir, { recursive: true });
  await writeFile(join(specsDir, `${missionId}.json`), JSON.stringify(spec, null, 2));
}

/**
 * Write a v1 Spec JSON (no runtime_signals, no rollout_plan) to the spec store path.
 * The schema_version field is 1 so migration is triggered at read time.
 */
async function writeSpecV1(
  dir: string,
  missionId: string,
): Promise<void> {
  const specsDir = join(dir, ".maestro", "specs");
  await mkdir(specsDir, { recursive: true });
  const spec = {
    schema_version: 1,
    mission_id: missionId,
    acceptance_criteria: [],
    non_goals: [],
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  };
  await writeFile(join(specsDir, `${missionId}.json`), JSON.stringify(spec, null, 2));
}

/**
 * Write a full v2 Spec with rollout_plan configured for the happy-path deploy gate.
 */
async function writeFullSpec(dir: string, missionId: string): Promise<void> {
  await writeSpec(dir, missionId, {
    schema_version: 2,
    mission_id: missionId,
    acceptance_criteria: [],
    non_goals: [],
    runtime_signals: [],
    rollout_plan: {
      feature_flag: "ff_deploy_safe",
      canary: { stages: [{ percent: 10, hold_minutes: 5 }] },
      rollback_command: "echo rollback",
    },
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  });
}

/**
 * Write owners.yaml as a git-committed file on the current branch.
 * The deploy gate loads owners from the base ref via `git show <base>:...`
 * so we commit it and pass --base HEAD or HEAD~1 accordingly.
 */
async function commitOwnersYaml(dir: string, deployApprovers: string[]): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });

  const approverLines = deployApprovers.length > 0
    ? deployApprovers.map((u) => `  - ${u}`).join("\n")
    : "";

  const content = [
    "policy_approver:",
    "  - admin",
    "ratchet_approver:",
    "  - admin",
    "sensitive_waiver:",
    "  - admin",
    ...(approverLines.length > 0 ? ["deploy_approver:", approverLines] : []),
  ].join("\n");

  await writeFile(join(policyDir, "owners.yaml"), content);
  await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
  await runCommand(
    ["git", "commit", "-m", "chore: owners.yaml", "--author", "Test <test@example.com>"],
    dir,
  );
}

/**
 * Seed a rollback-exercised evidence row directly to the evidence store.
 */
async function seedRollbackEvidence(
  dir: string,
  taskId: string,
  witnessLevel: "witnessed-by-ci" | "witnessed-by-maestro" | "agent-claimed-locally",
): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });

  const ts = String(Date.now()).padStart(13, "0");
  // Suffix must be exactly 6 hex chars (pattern: /^evd-\d{13}-[0-9a-f]{6}$/)
  const id = `evd-${ts}-ab0001`;

  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "rollback-exercised",
    witness_level: witnessLevel,
    created_at: new Date().toISOString(),
    payload: {
      command: "echo rollback-seeded",
      exit: 0,
    },
  };

  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

/**
 * Read all evidence rows for a task from the file system.
 */
async function readEvidenceRows(
  dir: string,
  taskId: string,
): Promise<Array<Record<string, unknown>>> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  let files: string[];
  try {
    const { readdir } = await import("node:fs/promises");
    files = await readdir(evidenceDir);
  } catch {
    return [];
  }
  const rows: Array<Record<string, unknown>> = [];
  for (const f of files) {
    if (!f.endsWith(".json")) continue;
    const raw = await readFile(join(evidenceDir, f), "utf8");
    rows.push(JSON.parse(raw) as Record<string, unknown>);
  }
  return rows;
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
 * Seed a deploy-readiness Evidence row with gate=pass directly.
 */
async function seedDeployReadinessPass(dir: string, taskId: string): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });

  const ts = String(Date.now()).padStart(13, "0");
  // Suffix must be exactly 6 hex chars (pattern: /^evd-\d{13}-[0-9a-f]{6}$/)
  const id = `evd-${ts}-d00001`;

  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "deploy-readiness",
    witness_level: "agent-claimed-locally",
    created_at: new Date().toISOString(),
    payload: {
      task_id: taskId,
      gate: "pass",
      checks: {
        feature_flag: { ok: true, value: "ff_deploy_safe" },
        canary_plan: { ok: true, stages: 1 },
        rollback: { ok: true, witness_evidence_id: "evd-0000000000000-ab0001" },
        owner: { ok: true, approvers: ["reina"] },
      },
    },
  };

  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

/**
 * Seed a contract for a task. Copied from L6 E2E convention.
 */
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
    intent: "L7 e2e test",
    scope: {
      filesExpected: opts.filesExpected,
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "l7-e2e-test",
    lockedBy: "l7-e2e-test",
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

// ─── Prometheus fixture ────────────────────────────────────────────────────────

/**
 * Spin up a minimal Bun.serve Prometheus-compatible fixture.
 * Returns { server, port, requestLog }.
 * The fixture responds to /api/v1/query with the configured metric value.
 */
function startPrometheusFixture(metricValue: string): {
  server: Server;
  port: number;
  requestLog: string[];
} {
  const requestLog: string[] = [];

  const server = Bun.serve({
    port: 0, // OS-assigned
    fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === "/api/v1/query") {
        requestLog.push(req.url);
        return Response.json({
          status: "success",
          data: {
            resultType: "vector",
            result: [[1700000000, metricValue]],
          },
        });
      }
      return new Response("not found", { status: 404 });
    },
  });

  return { server, port: server.port, requestLog };
}

// ─── Scenarios ─────────────────────────────────────────────────────────────────

describe("L7 deploy-safety flow (compiled binary)", () => {
  // ── S1: Spec v1 → v2 read migration ─────────────────────────────────────────

  it(
    "S1 Spec v1 → v2 read: CLI outputs schema_version:2; file on disk stays at schema_version:1",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "2026-01-l7-s1";
      await writeSpecV1(dir, missionId);

      // Run spec show — should succeed (v1 migrated at read time)
      const result = await runCompiled(
        ["spec", "show", "--mission", missionId, "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);
      const spec = expectJson<Record<string, unknown>>(result);

      // Output reflects v2 shape
      expect(spec["schema_version"]).toBe(2);
      expect(spec["mission_id"]).toBe(missionId);
      expect(Array.isArray(spec["runtime_signals"])).toBe(true);
      expect((spec["runtime_signals"] as unknown[]).length).toBe(0);

      // File on disk is NOT rewritten — still has schema_version: 1
      const specsDir = join(dir, ".maestro", "specs");
      const diskContent = await readFile(join(specsDir, `${missionId}.json`), "utf8");
      const diskSpec = JSON.parse(diskContent) as Record<string, unknown>;
      expect(diskSpec["schema_version"]).toBe(1);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S2: Witnessed rollback (local) ───────────────────────────────────────────

  it(
    "S2 Witnessed rollback (local): exit 0; rollback-exercised Evidence at witnessed-by-maestro",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "L7 rollback local");

      const result = await runCompiled(
        ["deploy", "rollback", "--task", taskId, "--command", "echo ok"],
        dir,
      );

      expect(result.exitCode).toBe(0);

      const rows = await readEvidenceRows(dir, taskId);
      const rollbackRows = rows.filter((r) => r["kind"] === "rollback-exercised");
      expect(rollbackRows.length).toBe(1);

      const row = rollbackRows[0]!;
      expect(row["witness_level"]).toBe("witnessed-by-maestro");
      const payload = row["payload"] as Record<string, unknown>;
      expect(payload["command"]).toBe("echo ok");
      expect(payload["exit"]).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S3: Witnessed rollback (CI) ──────────────────────────────────────────────

  it(
    "S3 Witnessed rollback (CI): GITHUB_ACTIONS=true; witness is witnessed-by-ci",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const taskId = await createTask(dir, "L7 rollback ci");

      const result = await runCompiled(
        ["deploy", "rollback", "--task", taskId, "--command", "echo ok"],
        dir,
        { env: { GITHUB_ACTIONS: "true" } },
      );

      expect(result.exitCode).toBe(0);

      const rows = await readEvidenceRows(dir, taskId);
      const rollbackRows = rows.filter((r) => r["kind"] === "rollback-exercised");
      expect(rollbackRows.length).toBe(1);

      const row = rollbackRows[0]!;
      expect(row["witness_level"]).toBe("witnessed-by-ci");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S4: Deploy gate happy path ────────────────────────────────────────────────

  it(
    "S4 Deploy gate happy path: all 4 checks pass; gate=pass; exit 0; deploy-readiness Evidence written",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "2026-01-l7-s4";
      await writeFullSpec(dir, missionId);

      const taskId = await createTaskWithMission(dir, "L7 gate happy", missionId);

      // Seed rollback at witnessed-by-ci
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      // Commit owners.yaml with deploy_approver
      await commitOwnersYaml(dir, ["reina"]);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);
      const output = expectJson<{
        gate: string;
        checks: {
          feature_flag: { ok: boolean };
          canary_plan: { ok: boolean };
          rollback: { ok: boolean };
          owner: { ok: boolean };
        };
      }>(result);

      expect(output.gate).toBe("pass");
      expect(output.checks.feature_flag.ok).toBe(true);
      expect(output.checks.canary_plan.ok).toBe(true);
      expect(output.checks.rollback.ok).toBe(true);
      expect(output.checks.owner.ok).toBe(true);

      // Evidence row written
      const rows = await readEvidenceRows(dir, taskId);
      const drRows = rows.filter((r) => r["kind"] === "deploy-readiness");
      expect(drRows.length).toBe(1);
      const drPayload = drRows[0]!["payload"] as Record<string, unknown>;
      expect(drPayload["gate"]).toBe("pass");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S5: Deploy gate missing rollback ──────────────────────────────────────────

  it(
    "S5 Deploy gate missing rollback: rollback.ok=false; gate=fail; exit 1",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "2026-01-l7-s5";
      await writeFullSpec(dir, missionId);

      const taskId = await createTaskWithMission(dir, "L7 gate no rollback", missionId);

      // No rollback evidence seeded

      await commitOwnersYaml(dir, ["reina"]);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{
        gate: string;
        checks: {
          feature_flag: { ok: boolean };
          canary_plan: { ok: boolean };
          rollback: { ok: boolean };
          owner: { ok: boolean };
        };
      }>(result);

      expect(output.gate).toBe("fail");
      expect(output.checks.rollback.ok).toBe(false);
      // Other three checks pass
      expect(output.checks.feature_flag.ok).toBe(true);
      expect(output.checks.canary_plan.ok).toBe(true);
      expect(output.checks.owner.ok).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S6: Deploy gate missing flag ──────────────────────────────────────────────

  it(
    "S6 Deploy gate missing flag: feature_flag.ok=false; gate=fail; exit 1",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "2026-01-l7-s6";
      // Spec with no feature_flag
      await writeSpec(dir, missionId, {
        schema_version: 2,
        mission_id: missionId,
        acceptance_criteria: [],
        non_goals: [],
        runtime_signals: [],
        rollout_plan: {
          // no feature_flag
          canary: { stages: [{ percent: 10, hold_minutes: 5 }] },
          rollback_command: "echo rollback",
        },
        created_at: "2026-01-01T00:00:00.000Z",
        updated_at: "2026-01-01T00:00:00.000Z",
      });

      const taskId = await createTaskWithMission(dir, "L7 gate no flag", missionId);
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");
      await commitOwnersYaml(dir, ["reina"]);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{
        gate: string;
        checks: { feature_flag: { ok: boolean }; canary_plan: { ok: boolean }; rollback: { ok: boolean }; owner: { ok: boolean } };
      }>(result);

      expect(output.gate).toBe("fail");
      expect(output.checks.feature_flag.ok).toBe(false);
      expect(output.checks.canary_plan.ok).toBe(true);
      expect(output.checks.rollback.ok).toBe(true);
      expect(output.checks.owner.ok).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S7: Deploy gate missing owner ─────────────────────────────────────────────

  it(
    "S7 Deploy gate missing owner: owner.ok=false; gate=fail; exit 1",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "2026-01-l7-s7";
      await writeFullSpec(dir, missionId);

      const taskId = await createTaskWithMission(dir, "L7 gate no owner", missionId);
      await seedRollbackEvidence(dir, taskId, "witnessed-by-ci");

      // Commit owners.yaml with empty deploy_approver
      await commitOwnersYaml(dir, []);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      expect(result.exitCode).toBe(1);
      const output = expectJson<{
        gate: string;
        checks: { feature_flag: { ok: boolean }; canary_plan: { ok: boolean }; rollback: { ok: boolean }; owner: { ok: boolean } };
      }>(result);

      expect(output.gate).toBe("fail");
      expect(output.checks.owner.ok).toBe(false);
      expect(output.checks.feature_flag.ok).toBe(true);
      expect(output.checks.canary_plan.ok).toBe(true);
      expect(output.checks.rollback.ok).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S8: Runtime check happy path ──────────────────────────────────────────────

  it(
    "S8 Runtime check happy path: value 0.42 < threshold 1.0; pass=true; one runtime-signal Evidence row",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const { server, port, requestLog } = startPrometheusFixture("0.42");
      servers.push(server);

      const missionId = "2026-01-l7-s8";
      await writeSpec(dir, missionId, {
        schema_version: 2,
        mission_id: missionId,
        acceptance_criteria: [],
        non_goals: [],
        runtime_signals: [
          {
            name: "p99",
            provider: "prometheus",
            query: "histogram_quantile(0.99,sum(rate(http_request_duration_seconds_bucket[5m])) by (le))",
            threshold: { operator: "<", value: 1.0 },
            severity: "warn",
          },
        ],
        created_at: "2026-01-01T00:00:00.000Z",
        updated_at: "2026-01-01T00:00:00.000Z",
      });

      const taskId = await createTaskWithMission(dir, "L7 runtime happy", missionId);

      const result = await runCompiled(
        ["runtime", "check", "--task", taskId,
          "--provider-base-url", `http://localhost:${port}`,
          "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);
      const output = expectJson<{
        outcomes: Array<{ signal_name: string; pass: boolean; note?: string }>;
      }>(result);

      expect(output.outcomes.length).toBe(1);
      expect(output.outcomes[0]!.signal_name).toBe("p99");
      expect(output.outcomes[0]!.pass).toBe(true);
      expect(output.outcomes[0]!.note).toBeUndefined();

      // Evidence row written with pass=true and value=0.42
      const rows = await readEvidenceRows(dir, taskId);
      const signalRows = rows.filter((r) => r["kind"] === "runtime-signal");
      expect(signalRows.length).toBe(1);
      const payload = signalRows[0]!["payload"] as Record<string, unknown>;
      expect(payload["pass"]).toBe(true);
      expect(payload["value"]).toBe(0.42);
      expect(payload["signal_name"]).toBe("p99");

      // Prometheus fixture received requests
      expect(requestLog.length).toBeGreaterThan(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S9: Runtime check threshold breach ───────────────────────────────────────

  it(
    "S9 Runtime check threshold breach: value 1.5 >= threshold 1.0; pass=false",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const { server, port } = startPrometheusFixture("1.5");
      servers.push(server);

      const missionId = "2026-01-l7-s9";
      await writeSpec(dir, missionId, {
        schema_version: 2,
        mission_id: missionId,
        acceptance_criteria: [],
        non_goals: [],
        runtime_signals: [
          {
            name: "p99",
            provider: "prometheus",
            query: "histogram_quantile(0.99,sum(rate(http_request_duration_seconds_bucket[5m])) by (le))",
            threshold: { operator: "<", value: 1.0 },
            severity: "warn",
          },
        ],
        created_at: "2026-01-01T00:00:00.000Z",
        updated_at: "2026-01-01T00:00:00.000Z",
      });

      const taskId = await createTaskWithMission(dir, "L7 runtime breach", missionId);

      const result = await runCompiled(
        ["runtime", "check", "--task", taskId,
          "--provider-base-url", `http://localhost:${port}`,
          "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);
      const output = expectJson<{
        outcomes: Array<{ signal_name: string; pass: boolean }>;
      }>(result);

      expect(output.outcomes.length).toBe(1);
      expect(output.outcomes[0]!.pass).toBe(false);

      const rows = await readEvidenceRows(dir, taskId);
      const signalRows = rows.filter((r) => r["kind"] === "runtime-signal");
      expect(signalRows.length).toBe(1);
      const payload = signalRows[0]!["payload"] as Record<string, unknown>;
      expect(payload["pass"]).toBe(false);
      expect(payload["value"]).toBe(1.5);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S10: Runtime check unsupported provider ───────────────────────────────────

  it(
    "S10 Runtime check unsupported provider: datadog; pass=false; note=unsupported provider; no HTTP requests",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      // Track requests to a fixture; we assert none arrive
      const { server, port, requestLog } = startPrometheusFixture("0.42");
      servers.push(server);

      const missionId = "2026-01-l7-s10";
      await writeSpec(dir, missionId, {
        schema_version: 2,
        mission_id: missionId,
        acceptance_criteria: [],
        non_goals: [],
        runtime_signals: [
          {
            name: "custom_metric",
            provider: "datadog",
            query: "avg:http.latency{*}",
            threshold: { operator: "<", value: 1.0 },
            severity: "warn",
          },
        ],
        created_at: "2026-01-01T00:00:00.000Z",
        updated_at: "2026-01-01T00:00:00.000Z",
      });

      const taskId = await createTaskWithMission(dir, "L7 runtime unsupported", missionId);

      // Don't pass --provider-base-url; the CLI should never call the fixture
      const result = await runCompiled(
        ["runtime", "check", "--task", taskId,
          "--provider-base-url", `http://localhost:${port}`,
          "--json"],
        dir,
      );

      expect(result.exitCode).toBe(0);

      // The CLI emits `[skip] provider datadog not supported` via console.log
      // before the JSON block. Extract the JSON portion from stdout.
      const jsonStart = result.stdout.indexOf("{");
      const jsonStr = jsonStart >= 0 ? result.stdout.slice(jsonStart) : result.stdout;
      const output = JSON.parse(jsonStr) as {
        outcomes: Array<{ signal_name: string; pass: boolean; note?: string }>;
      };

      expect(output.outcomes.length).toBe(1);
      expect(output.outcomes[0]!.pass).toBe(false);
      expect(output.outcomes[0]!.note).toBe("unsupported provider");

      // Evidence row written with pass=false and note
      const rows = await readEvidenceRows(dir, taskId);
      const signalRows = rows.filter((r) => r["kind"] === "runtime-signal");
      expect(signalRows.length).toBe(1);
      const payload = signalRows[0]!["payload"] as Record<string, unknown>;
      expect(payload["pass"]).toBe(false);
      expect(payload["note"]).toBe("unsupported provider");

      // The fixture should NOT have received any requests because the provider
      // is skipped before any HTTP call
      expect(requestLog.length).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S11: L7.9 deploy authorized ──────────────────────────────────────────────

  it(
    "S11 L7.9 deploy authorized: PR author in deploy_approver; CI check conclusion does not contain deploy not authorized",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      const missionId = "2026-01-l7-s11";
      await writeFullSpec(dir, missionId);

      // Step 1: commit owners.yaml with reina as deploy_approver.
      // ci verify with --base HEAD~1 loads owners from this commit.
      await commitOwnersYaml(dir, ["reina"]);

      // Step 2: commit the feature file — this becomes HEAD.
      // diff vs HEAD~1 = only the feature file.
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Create task with missionId
      const taskId = await createTaskWithMission(dir, "L7 ci authorized", missionId);

      // Seed contract so verdict request doesn't fail
      await seedContract(dir, taskId, { filesExpected: ["src/feature.ts"], riskClass: "low" });

      // Seed a deploy-readiness Evidence row with gate=pass
      await seedDeployReadinessPass(dir, taskId);

      // PR author is reina — in deploy_approver
      shim.setPrAuthor("reina");

      const githubOutputFile = join(dir, "github-output.txt");
      const eventFile = await writeEventFile(dir, 42);
      const ciEnv = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 42,
        shimBinDir: shim.binDir,
        githubOutputFile,
        eventFile,
      });

      await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "42", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv },
      );

      const state = shim.readState();
      expect(state.checkRuns.length).toBeGreaterThan(0);

      const lastRun = state.checkRuns[state.checkRuns.length - 1]!;
      // Deploy is authorized — summary must NOT contain "deploy not authorized"
      const summary = lastRun.output?.summary ?? "";
      expect(summary).not.toContain("deploy not authorized");

      // prLookupCalls should record the call for PR 42
      expect(state.prLookupCalls.length).toBeGreaterThan(0);
      expect(state.prLookupCalls.some((c) => c.pr === 42)).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  // ── S12: L7.9 deploy not-authorized ──────────────────────────────────────────

  it(
    "S12 L7.9 deploy not-authorized: PR author NOT in deploy_approver; CI check conclusion=failure; summary contains deploy not authorized",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const shim = await createFakeGhShim();
      shims.push(shim);

      const missionId = "2026-01-l7-s12";
      await writeFullSpec(dir, missionId);

      // Step 1: commit owners.yaml with only reina as deploy_approver (not someoneelse)
      await commitOwnersYaml(dir, ["reina"]);

      // Step 2: commit the feature file — this becomes HEAD
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const taskId = await createTaskWithMission(dir, "L7 ci not authorized", missionId);

      // Seed contract so verdict request doesn't fail
      await seedContract(dir, taskId, { filesExpected: ["src/feature.ts"], riskClass: "low" });

      // Seed a deploy-readiness Evidence row with gate=pass
      await seedDeployReadinessPass(dir, taskId);

      // PR author is someoneelse — NOT in deploy_approver
      shim.setPrAuthor("someoneelse");

      const githubOutputFile = join(dir, "github-output.txt");
      const eventFile = await writeEventFile(dir, 43);
      const ciEnv = await buildCiEnv(dir, {
        repo: "fixture/repo",
        pr: 43,
        shimBinDir: shim.binDir,
        githubOutputFile,
        eventFile,
      });

      await runCompiled(
        ["ci", "verify", "--task", taskId, "--pr", "43", "--base", "HEAD~1", "--json"],
        dir,
        { env: ciEnv },
      );

      const state = shim.readState();
      expect(state.checkRuns.length).toBeGreaterThan(0);

      const lastRun = state.checkRuns[state.checkRuns.length - 1]!;
      // Deploy block reason forces conclusion to failure
      expect(lastRun.conclusion).toBe("failure");

      const summary = lastRun.output?.summary ?? "";
      expect(summary).toContain("deploy not authorized");
      expect(summary).toContain("someoneelse");
      expect(summary).toContain("deploy_approver");

      // prLookupCalls should record the call for PR 43
      expect(state.prLookupCalls.some((c) => c.pr === 43)).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
