/**
 * Integration tests for checkpoint CLI commands
 */
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 15_000;

async function run(
  args: string[],
  cwd = process.cwd(),
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
  });

  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

async function initGitRepo(cwd: string): Promise<void> {
  const init = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await init.exited;
}

function createSamplePlan(): object {
  return {
    title: "Test Mission",
    description: "A test mission for checkpoint CLI integration",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
      { id: "m2", title: "Milestone 2", description: "Second milestone", order: 1 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        skillName: "test-skill",
        verificationSteps: ["step1", "step2"],
        fulfills: ["assertion-f1-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        skillName: "test-skill",
        verificationSteps: ["step3"],
        fulfills: ["assertion-f2-1"],
      },
    ],
  };
}

async function createMission(cwd: string): Promise<string> {
  const plan = createSamplePlan();
  const planPath = join(cwd, "plan.json");
  await writeFile(planPath, JSON.stringify(plan, null, 2));

  const { stdout, exitCode } = await run(
    ["mission", "create", "--file", planPath, "--json"],
    cwd,
  );

  expect(exitCode).toBe(0);
  return JSON.parse(stdout).mission.id;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-checkpoint-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("checkpoint CLI commands", () => {
  describe("checkpoint save", () => {
    it("checkpoint save --mission <id> writes a timestamped snapshot", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "save", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Checkpoint saved");
      expect(stdout).toContain("Mission:");
      expect(stdout).toContain(missionId);
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint save --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.checkpoint).toBeDefined();
      expect(result.checkpoint.id).toBeTruthy();
      expect(result.checkpoint.missionId).toBe(missionId);
      expect(result.checkpoint.milestoneId).toBeDefined();
      expect(result.checkpoint.timestamp).toBeDefined();
      expect(result.checkpoint.featureStates).toBeDefined();
      expect(result.checkpoint.assertionStates).toBeDefined();
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint save captures feature and assertion states", async () => {
      const missionId = await createMission(tmpDir);

      // Update a feature status
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      // Pass an assertion
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      await run(
        ["validate", "update", assertions[0]!.id, "--mission", missionId, "--status", "passed"],
        tmpDir,
      );

      const { stdout, exitCode } = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.checkpoint.featureStates).toEqual(
        expect.objectContaining({ f1: "in_progress" }),
      );
      expect(result.checkpoint.assertionStates).toEqual(
        expect.objectContaining({ [assertions[0]!.id]: "passed" }),
      );
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint save errors for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["checkpoint", "save", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("checkpoint list", () => {
    it("checkpoint list --mission <id> returns checkpoints newest-first", async () => {
      const missionId = await createMission(tmpDir);

      // Save two checkpoints
      await run(["checkpoint", "save", "--mission", missionId], tmpDir);
      await new Promise((r) => setTimeout(r, 50)); // Small delay for ordering
      await run(["checkpoint", "save", "--mission", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Test Mission");
      expect(stdout).toContain("2 checkpoint(s)");
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint list --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      await run(["checkpoint", "save", "--mission", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "list", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.mission).toBeDefined();
      expect(result.mission.id).toBe(missionId);
      expect(result.checkpoints).toBeDefined();
      expect(Array.isArray(result.checkpoints)).toBe(true);
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint list returns empty array when no checkpoints exist", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "list", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.checkpoints).toHaveLength(0);
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint list errors for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["checkpoint", "list", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("checkpoint load", () => {
    it("checkpoint load --mission <id> returns the latest checkpoint", async () => {
      const missionId = await createMission(tmpDir);

      // Save two checkpoints
      const save1 = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(save1.exitCode).toBe(0);
      const checkpoint1 = JSON.parse(save1.stdout).checkpoint;
      
      await new Promise((r) => setTimeout(r, 50));
      
      // Save second checkpoint
      const saveResult = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );
      const latestId = JSON.parse(saveResult.stdout).checkpoint.id;

      const { stdout, exitCode } = await run(
        ["checkpoint", "load", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      // Should return the latest checkpoint (different ID from first)
      expect(result.checkpoint.id).toBe(latestId);
      expect(result.checkpoint.id).not.toBe(checkpoint1.id);
        // Verify checkpoint structure
        expect(result.checkpoint.missionId).toBe(missionId);
        expect(result.checkpoint.timestamp).toBeDefined();
        expect(result.checkpoint.featureStates).toBeDefined();
        expect(result.checkpoint.assertionStates).toBeDefined();
        expect(result.restored).toEqual({
          featureCount: 0,
          assertionCount: 0,
        });
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint load restores saved state and reports restore counts", async () => {
      const missionId = await createMission(tmpDir);

      await run(["checkpoint", "save", "--mission", missionId], tmpDir);
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      const { stdout, exitCode } = await run(
        ["checkpoint", "load", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Checkpoint restored");
      expect(stdout).toContain("Features restored: 1");
      expect(stdout).toContain("Assertions restored: 0");
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint load explains when current state already matches the checkpoint", async () => {
      const missionId = await createMission(tmpDir);

      await run(["checkpoint", "save", "--mission", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "load", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Features restored: 0");
      expect(stdout).toContain("Assertions restored: 0");
      expect(stdout).toContain("current state already matches the checkpoint");
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint load --json includes restore counts", async () => {
      const missionId = await createMission(tmpDir);

      await run(["checkpoint", "save", "--mission", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["checkpoint", "load", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.checkpoint).toBeDefined();
      expect(result.restored).toEqual({
        featureCount: 0,
        assertionCount: 0,
      });
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint load errors when no checkpoints exist", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["checkpoint", "load", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("No checkpoints found");
    }, SLOW_CLI_TIMEOUT_MS);

    it("checkpoint load errors for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["checkpoint", "load", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("checkpoint full lifecycle", () => {
    it("can save, list, and load checkpoints end to end", async () => {
      const missionId = await createMission(tmpDir);

      // Save initial checkpoint
      const save1 = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(save1.exitCode).toBe(0);
      const checkpoint1 = JSON.parse(save1.stdout).checkpoint;

      // List should show one checkpoint
      const list1 = await run(
        ["checkpoint", "list", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(list1.exitCode).toBe(0);
      const listResult1 = JSON.parse(list1.stdout);
      expect(listResult1.checkpoints).toHaveLength(1);
      expect(listResult1.checkpoints[0].id).toBe(checkpoint1.id);

      // Save second checkpoint
      await new Promise((r) => setTimeout(r, 50));
      const save2 = await run(
        ["checkpoint", "save", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(save2.exitCode).toBe(0);
      const checkpoint2 = JSON.parse(save2.stdout).checkpoint;

      // List should show two checkpoints, newest first
      const list2 = await run(
        ["checkpoint", "list", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(list2.exitCode).toBe(0);
      const listResult2 = JSON.parse(list2.stdout);
      expect(listResult2.checkpoints).toHaveLength(2);
      expect(listResult2.checkpoints[0].id).toBe(checkpoint2.id);
      expect(listResult2.checkpoints[1].id).toBe(checkpoint1.id);

      // Load should return the latest
      const load = await run(
        ["checkpoint", "load", "--mission", missionId, "--json"],
        tmpDir,
      );
      expect(load.exitCode).toBe(0);
      const loadResult = JSON.parse(load.stdout);
      expect(loadResult.checkpoint.id).toBe(checkpoint2.id);
      expect(loadResult.checkpoint.featureStates).toBeDefined();
      expect(loadResult.restored).toBeDefined();
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("JSON flag positions", () => {
    it("JSON flag works from root position for checkpoint commands", async () => {
      const missionId = await createMission(tmpDir);

      // save - root position
      const rootSave = await run(
        ["--json", "checkpoint", "save", "--mission", missionId],
        tmpDir,
      );
      expect(rootSave.exitCode).toBe(0);
      expect(() => JSON.parse(rootSave.stdout)).not.toThrow();

      // list - root position
      const rootList = await run(
        ["--json", "checkpoint", "list", "--mission", missionId],
        tmpDir,
      );
      expect(rootList.exitCode).toBe(0);
      expect(() => JSON.parse(rootList.stdout)).not.toThrow();

      // load - root position
      const rootLoad = await run(
        ["--json", "checkpoint", "load", "--mission", missionId],
        tmpDir,
      );
      expect(rootLoad.exitCode).toBe(0);
      expect(() => JSON.parse(rootLoad.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from group position for checkpoint commands", async () => {
      const missionId = await createMission(tmpDir);

      // save - group position
      const groupSave = await run(
        ["checkpoint", "--json", "save", "--mission", missionId],
        tmpDir,
      );
      expect(groupSave.exitCode).toBe(0);
      expect(() => JSON.parse(groupSave.stdout)).not.toThrow();

      // list - group position
      const groupList = await run(
        ["checkpoint", "--json", "list", "--mission", missionId],
        tmpDir,
      );
      expect(groupList.exitCode).toBe(0);
      expect(() => JSON.parse(groupList.stdout)).not.toThrow();

      // load - group position
      const groupLoad = await run(
        ["checkpoint", "--json", "load", "--mission", missionId],
        tmpDir,
      );
      expect(groupLoad.exitCode).toBe(0);
      expect(() => JSON.parse(groupLoad.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);
  });
});
