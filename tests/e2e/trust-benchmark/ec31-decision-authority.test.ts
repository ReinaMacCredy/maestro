/**
 * EC 31 — Decision authority not enforced.
 *
 * Mitigation: owners.yaml deploy_approver role at L7.9 (deploy gate owner check).
 *
 * The deploy gate owner check passes only when at least one deploy_approver is
 * listed in owners.yaml. Without a deploy_approver entry, the owner check fails.
 *
 * Positive: owners.yaml has no deploy_approver → deploy gate owner check fails
 *           (owner.ok=false); gate=fail; exit 1.
 * Negative: owners.yaml has at least one deploy_approver → owner check passes
 *           (owner.ok=true when all other gate checks also pass).
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, readFile, writeFile } from "node:fs/promises";
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec31-"));
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

/**
 * Link a task to a mission by patching tasks.jsonl (deploy gate needs a missionId
 * for the Spec/rollout_plan).
 */
async function linkTaskToMission(dir: string, taskId: string, missionId: string): Promise<void> {
  const jsonlPath = join(dir, ".maestro", "tasks", "tasks.jsonl");
  const content = await readFile(jsonlPath, "utf8");
  const lines = content.split("\n").filter((l) => l.trim().length > 0);
  const patched = lines.map((line) => {
    const obj = JSON.parse(line) as Record<string, unknown>;
    if (obj["id"] === taskId) return JSON.stringify({ ...obj, missionId });
    return line;
  });
  await writeFile(jsonlPath, patched.join("\n") + "\n");
}

/** Write a v2 Spec with rollout_plan populated. */
async function writeFullSpec(dir: string, missionId: string): Promise<void> {
  const specsDir = join(dir, ".maestro", "specs");
  await mkdir(specsDir, { recursive: true });
  const spec = {
    schema_version: 2,
    mission_id: missionId,
    acceptance_criteria: [],
    non_goals: [],
    runtime_signals: [],
    rollout_plan: {
      feature_flag: "ff_ec31",
      canary: { stages: [{ percent: 10, hold_minutes: 5 }] },
      rollback_command: "echo rollback",
    },
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  };
  await writeFile(join(specsDir, `${missionId}.json`), JSON.stringify(spec, null, 2));
}

/**
 * Commit owners.yaml to git with optional deploy_approver entries.
 * The deploy gate loads owners from the base ref via `git show <base>:...`
 * so it must be committed.
 */
async function commitOwnersYaml(
  dir: string,
  deployApprovers: string[],
): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  const lines = [
    "policy_approver:",
    "  - admin",
    "ratchet_approver:",
    "  - admin",
    "sensitive_waiver:",
    "  - admin",
    ...(deployApprovers.length > 0
      ? ["deploy_approver:", ...deployApprovers.map((u) => `  - ${u}`)]
      : []),
  ];
  await writeFile(join(policyDir, "owners.yaml"), lines.join("\n"));
  await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
  await runCommand(
    ["git", "commit", "-m", "chore: owners.yaml", "--author", "Test <test@example.com>"],
    dir,
  );
}

/**
 * Seed a rollback-exercised Evidence row at witnessed-by-ci so the rollback
 * check in deploy gate passes.
 */
async function seedRollbackEvidence(dir: string, taskId: string): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });
  const ts = String(Date.now()).padStart(13, "0");
  const id = `evd-${ts}-ab0001`;
  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "rollback-exercised",
    witness_level: "witnessed-by-ci",
    created_at: new Date().toISOString(),
    payload: { command: "echo rollback", exit: 0 },
  };
  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("EC 31 — decision authority (owners.yaml deploy_approver at L7.9)", () => {
  it(
    "positive: no deploy_approver in owners.yaml → owner check fails; deploy gate exits 1",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "ec31-mission-a";
      const taskId = await createTask(dir, "EC31 no deploy_approver");
      await linkTaskToMission(dir, taskId, missionId);
      await writeFullSpec(dir, missionId);
      await seedRollbackEvidence(dir, taskId);

      // Commit owners.yaml WITHOUT deploy_approver
      await commitOwnersYaml(dir, []);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      // Gate fails (owner check fails)
      expect(result.exitCode).toBe(1);
      const gate = expectJson<{
        gate: string;
        checks: { owner: { ok: boolean } };
      }>(result);
      expect(gate.gate).toBe("fail");
      expect(gate.checks.owner.ok).toBe(false);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: owners.yaml has deploy_approver → owner check passes; all other checks pass → gate=pass",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "ec31-mission-b";
      const taskId = await createTask(dir, "EC31 with deploy_approver");
      await linkTaskToMission(dir, taskId, missionId);
      await writeFullSpec(dir, missionId);
      await seedRollbackEvidence(dir, taskId);

      // Commit owners.yaml WITH a deploy_approver
      await commitOwnersYaml(dir, ["deploy-bot"]);

      const result = await runCompiled(
        ["deploy", "gate", "--task", taskId, "--base", "HEAD", "--json"],
        dir,
      );

      // Gate passes when all 4 checks pass
      expect(result.exitCode).toBe(0);
      const gate = expectJson<{
        gate: string;
        checks: { owner: { ok: boolean } };
      }>(result);
      expect(gate.gate).toBe("pass");
      expect(gate.checks.owner.ok).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
