/**
 * EC 23 — Proof not tied to acceptance criteria.
 *
 * Mitigation: ProofMap at L3.5 (task proof command).
 *
 * buildProofMap joins Spec.acceptance_criteria with Evidence rows via criterion_id.
 * Evidence rows without a criterion_id do not cover any criterion. Uncovered
 * criteria are surfaced as uncoveredCount > 0.
 *
 * Positive: Spec has acceptance criteria; no evidence is linked to them →
 *           uncoveredCount equals the number of criteria.
 * Negative: Evidence is recorded with a matching criterion_id →
 *           uncoveredCount drops to 0.
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
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec23-"));
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
 * Link a task to a mission by patching tasks.jsonl.
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

/**
 * Write a v2 Spec with two acceptance criteria.
 */
async function writeSpec(
  dir: string,
  missionId: string,
  criteria: Array<{ id: string; text: string }>,
): Promise<void> {
  const specsDir = join(dir, ".maestro", "specs");
  await mkdir(specsDir, { recursive: true });
  const spec = {
    schema_version: 2,
    mission_id: missionId,
    acceptance_criteria: criteria,
    non_goals: [],
    runtime_signals: [],
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  };
  await writeFile(join(specsDir, `${missionId}.json`), JSON.stringify(spec, null, 2));
}

/**
 * Seed an evidence row that references a criterion_id.
 * Written directly to the file system to avoid relying on a criterion_id CLI flag
 * that may not exist — the evidence command doesn't expose criterion_id directly.
 */
async function seedCommandEvidenceWithCriterion(
  dir: string,
  taskId: string,
  criterionId: string,
): Promise<void> {
  const evidenceDir = join(dir, ".maestro", "evidence", taskId);
  await mkdir(evidenceDir, { recursive: true });
  const ts = String(Date.now()).padStart(13, "0");
  const id = `evd-${ts}-cc0001`;
  const row = {
    schema_version: 3,
    id,
    task_id: taskId,
    kind: "command",
    witness_level: "agent-claimed-locally",
    created_at: new Date().toISOString(),
    payload: {
      command: "bun test",
      exit: 0,
      criterion_id: criterionId,
    },
  };
  await writeFile(join(evidenceDir, `${id}.json`), JSON.stringify(row, null, 2));
}

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("EC 23 — proof not tied to acceptance criteria (ProofMap at L3.5)", () => {
  it(
    "positive: Spec criteria with no linked evidence → uncoveredCount equals total criteria",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "ec23-mission-a";
      const taskId = await createTask(dir, "EC23 proof not tied");
      await linkTaskToMission(dir, taskId, missionId);

      const criteria = [
        { id: "ac-0001", text: "Unit tests pass" },
        { id: "ac-0002", text: "Integration tests pass" },
      ];
      await writeSpec(dir, missionId, criteria);

      // No evidence linked to either criterion

      const result = await runCompiled(
        ["task", "proof", "--task", taskId, "--json"],
        dir,
      );
      expect(result.exitCode).toBe(0);
      const proofMap = expectJson<{
        taskId: string;
        missionId: string;
        entries: Array<{ criterionId: string; covered: boolean; evidence: unknown[] }>;
        uncoveredCount: number;
      }>(result);

      expect(proofMap.uncoveredCount).toBe(criteria.length);
      expect(proofMap.entries.every((e) => !e.covered)).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: evidence with matching criterion_id → that criterion is covered; uncoveredCount decreases",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      const missionId = "ec23-mission-b";
      const taskId = await createTask(dir, "EC23 proof tied");
      await linkTaskToMission(dir, taskId, missionId);

      const criteria = [
        { id: "ac-0001", text: "Unit tests pass" },
        { id: "ac-0002", text: "Integration tests pass" },
      ];
      await writeSpec(dir, missionId, criteria);

      // Link evidence to the first criterion
      await seedCommandEvidenceWithCriterion(dir, taskId, "ac-0001");

      const result = await runCompiled(
        ["task", "proof", "--task", taskId, "--json"],
        dir,
      );
      expect(result.exitCode).toBe(0);
      const proofMap = expectJson<{
        entries: Array<{ criterionId: string; covered: boolean }>;
        uncoveredCount: number;
      }>(result);

      // ac-0001 covered; ac-0002 still uncovered
      expect(proofMap.uncoveredCount).toBe(1);
      const ac1 = proofMap.entries.find((e) => e.criterionId === "ac-0001");
      const ac2 = proofMap.entries.find((e) => e.criterionId === "ac-0002");
      expect(ac1?.covered).toBe(true);
      expect(ac2?.covered).toBe(false);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
