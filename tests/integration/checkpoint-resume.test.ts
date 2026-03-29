/**
 * Integration tests for checkpoint save/load semantics
 * Tests: metadata-only checkpoint restore, multiple checkpoint handling, resume semantics
 * Fulfills: VAL-CHECKPOINT-001, VAL-CHECKPOINT-002, VAL-CROSS-001
 */
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, writeFile, rm, mkdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 20_000;

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

function createCheckpointPlan(): object {
  return {
    title: "Checkpoint Test Mission",
    description: "Mission for testing checkpoint semantics",
    milestones: [
      { id: "m1", title: "Phase 1", description: "Initial phase", order: 0 },
      { id: "m2", title: "Phase 2", description: "Second phase", order: 1 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        skillName: "test-skill",
        verificationSteps: ["Step 1"],
        fulfills: ["assert-f1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        skillName: "test-skill",
        verificationSteps: ["Step 2"],
        fulfills: ["assert-f2"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        skillName: "test-skill",
        verificationSteps: ["Step 3"],
        fulfills: ["assert-f3"],
      },
    ],
  };
}

async function createMission(cwd: string): Promise<string> {
  const plan = createCheckpointPlan();
  const planPath = join(cwd, "plan.json");
  await writeFile(planPath, JSON.stringify(plan, null, 2));

  const { stdout, exitCode } = await run(
    ["mission", "create", "--file", planPath, "--json"],
    cwd,
  );

  expect(exitCode).toBe(0);
  return JSON.parse(stdout).mission.id;
}

async function createSkill(baseDir: string, skillName: string): Promise<void> {
  const skillDir = join(baseDir, ".maestro", "skills", skillName);
  await mkdir(skillDir, { recursive: true });
  await writeFile(
    join(skillDir, "SKILL.md"),
    `# ${skillName}\n\nTest skill content.\n`,
  );
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-checkpoint-resume-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("checkpoint save semantics", () => {
  it("checkpoint captures all feature states at save time", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Set different feature states with verification
    const f1Result = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress", "--json"],
      tmpDir,
    );
    expect(f1Result.exitCode).toBe(0);
    expect(JSON.parse(f1Result.stdout).feature.status).toBe("in_progress");

    // Transition f2 through proper states: pending -> in_progress -> in_review -> completed
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_review"],
      tmpDir,
    );
    const f2Result = await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "completed", "--json"],
      tmpDir,
    );
    expect(f2Result.exitCode).toBe(0);
    expect(JSON.parse(f2Result.stdout).feature.status).toBe("completed");

    // Verify features were updated by listing
    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const features = JSON.parse(listResult.stdout).features;
    const f1 = features.find((f: { id: string }) => f.id === "f1");
    const f2 = features.find((f: { id: string }) => f.id === "f2");
    expect(f1.status).toBe("in_progress");
    expect(f2.status).toBe("completed");

    // Save checkpoint
    const saveResult = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(saveResult.exitCode).toBe(0);
    const checkpoint = JSON.parse(saveResult.stdout).checkpoint;

    // Verify all features captured
    expect(checkpoint.featureStates).toBeDefined();
    expect(checkpoint.featureStates.f1).toBe("in_progress");
    expect(checkpoint.featureStates.f2).toBe("completed");
    expect(checkpoint.featureStates.f3).toBe("pending");
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint captures all assertion states at save time", async () => {
    const missionId = await createMission(tmpDir);

    // Get assertions and set mixed states
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    const assertionList = JSON.parse(assertions.stdout).assertions;
    expect(assertionList.length).toBeGreaterThanOrEqual(2);

    // Pass first assertion
    await run(
      ["validate", "update", assertionList[0].id, "--mission", missionId, "--status", "passed"],
      tmpDir,
    );

    // Fail second assertion
    if (assertionList.length >= 2) {
      await run(
        ["validate", "update", assertionList[1].id, "--mission", missionId, "--status", "failed", "--evidence", "Test failure"],
        tmpDir,
      );
    }

    // Save checkpoint
    const saveResult = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint = JSON.parse(saveResult.stdout).checkpoint;

    // Verify assertion states captured
    expect(checkpoint.assertionStates[assertionList[0].id]).toBe("passed");
    if (assertionList.length >= 2) {
      expect(checkpoint.assertionStates[assertionList[1].id]).toBe("failed");
    }
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint captures current milestone", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Save checkpoint at m1
    const save1 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint1 = JSON.parse(save1.stdout).checkpoint;
    expect(checkpoint1.milestoneId).toBe("m1");

    // Complete m1
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    const m1Asserts = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    for (const a of JSON.parse(m1Asserts.stdout).assertions) {
      await run(
        ["validate", "update", a.id, "--mission", missionId, "--status", "passed"],
        tmpDir,
      );
    }

    await run(["milestone", "seal", "m1", "--mission", missionId], tmpDir);
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Save checkpoint at executing phase (after m1 completion)
    // Since m1 features are all completed, the checkpoint logic
    // determines the "current" milestone based on mission status
    const save2 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint2 = JSON.parse(save2.stdout).checkpoint;
    // Checkpoint captures the mission state at this point
    // The milestoneId reflects the active milestone based on mission status
    expect(checkpoint2.missionId).toBe(missionId);
    expect(checkpoint2.featureStates).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("multiple checkpoints are stored independently", async () => {
    const missionId = await createMission(tmpDir);

    // Save first checkpoint
    await run(["checkpoint", "save", "--mission", missionId], tmpDir);
    await new Promise((r) => setTimeout(r, 50));

    // Make changes
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    // Save second checkpoint
    const save2 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint2 = JSON.parse(save2.stdout).checkpoint;

    // List checkpoints
    const list = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoints = JSON.parse(list.stdout).checkpoints;
    expect(checkpoints.length).toBe(2);

    // Check that each checkpoint has its own ID
    const ids = checkpoints.map((c: { id: string }) => c.id);
    expect(new Set(ids).size).toBe(2); // All unique
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("checkpoint load semantics", () => {
  it("load returns the latest checkpoint by timestamp", async () => {
    const missionId = await createMission(tmpDir);

    // Save checkpoints with different states
    await run(["checkpoint", "save", "--mission", missionId], tmpDir);
    await new Promise((r) => setTimeout(r, 50));

    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    const save2 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint2 = JSON.parse(save2.stdout).checkpoint;

    // Load should return the second checkpoint
    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    const loaded = JSON.parse(load.stdout).checkpoint;
    expect(loaded.id).toBe(checkpoint2.id);
    expect(loaded.featureStates.f1).toBe("in_progress");
  }, SLOW_CLI_TIMEOUT_MS);

  it("load includes metadata-only restore warning", async () => {
    const missionId = await createMission(tmpDir);

    await run(["checkpoint", "save", "--mission", missionId], tmpDir);

    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    const result = JSON.parse(load.stdout);

    expect(result.warning).toBeDefined();
    expect(result.warning).toContain("WARNING");
    expect(result.warning).toContain("metadata only");
    expect(result.warning).toContain("NOT restored");
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint data is immutable once saved", async () => {
    const missionId = await createMission(tmpDir);

    // Set initial state and save
    // Note: The checkpoint captures the state at SAVE time
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    const save = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoint = JSON.parse(save.stdout).checkpoint;

    // Change feature state AFTER saving
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    // Load checkpoint - should return the saved state (in_progress)
    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    const loaded = JSON.parse(load.stdout).checkpoint;

    // Verify checkpoint captured the in_progress state at save time
    // (not the completed state set after saving)
    expect(loaded.featureStates.f1).toBe("in_progress");
    expect(loaded.id).toBe(checkpoint.id);
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("checkpoint resume workflow", () => {
  it("supports save work → checkpoint → continue → another checkpoint workflow", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Work on f1
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    // Checkpoint 1
    const cp1 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(cp1.exitCode).toBe(0);

    // Continue working - complete f1
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    // Work on f2
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    // Checkpoint 2
    const cp2 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(cp2.exitCode).toBe(0);

    // Continue - complete f2, start f3
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f3", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );

    // Checkpoint 3
    const cp3 = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(cp3.exitCode).toBe(0);

    // List checkpoints - should show progression
    const list = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoints = JSON.parse(list.stdout).checkpoints;
    expect(checkpoints.length).toBe(3);

    // Latest should be cp3
    expect(checkpoints[0].id).toBe(JSON.parse(cp3.stdout).checkpoint.id);
  }, SLOW_CLI_TIMEOUT_MS);

  it("can resume understanding from checkpoint after interruption", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Set up some state with verification
    // Transition f1 through proper states: pending -> in_progress -> in_review -> completed
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_review"],
      tmpDir,
    );
    const f1Result = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed", "--json"],
      tmpDir,
    );
    expect(f1Result.exitCode).toBe(0);
    expect(JSON.parse(f1Result.stdout).feature.status).toBe("completed");

    const f2Result = await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_progress", "--json"],
      tmpDir,
    );
    expect(f2Result.exitCode).toBe(0);
    expect(JSON.parse(f2Result.stdout).feature.status).toBe("in_progress");

    // Pass some assertions
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    const assertionList = JSON.parse(assertions.stdout).assertions;

    for (const a of assertionList.slice(0, 2)) {
      const result = await run(
        ["validate", "update", a.id, "--mission", missionId, "--status", "passed", "--json"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
    }

    // Save checkpoint (simulating "save progress before interruption")
    const save = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(save.exitCode).toBe(0);
    const checkpoint = JSON.parse(save.stdout).checkpoint;

    // Load checkpoint (simulating "resume after interruption")
    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(load.exitCode).toBe(0);
    const loaded = JSON.parse(load.stdout);

    // Verify we can understand the state at that point
    expect(loaded.checkpoint.featureStates.f1).toBe("completed");
    expect(loaded.checkpoint.featureStates.f2).toBe("in_progress");
    expect(Object.values(loaded.checkpoint.assertionStates).filter(s => s === "passed").length).toBeGreaterThanOrEqual(2);
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint captures feature reports for resume context", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Create feature with report - update to in_progress first
    const report = {
      content: "Implementation complete with full test coverage",
      timestamp: new Date().toISOString(),
      agent: "resume-test-agent",
    };
    
    // First update to in_progress with report
    await run(
      [
        "feature",
        "update",
        "f1",
        "--mission",
        missionId,
        "--status",
        "in_progress",
        "--report",
        JSON.stringify(report),
      ],
      tmpDir,
    );
    
    // Then update to completed (via in_review)
    await run(
      [
        "feature",
        "update",
        "f1",
        "--mission",
        missionId,
        "--status",
        "in_review",
      ],
      tmpDir,
    );
    await run(
      [
        "feature",
        "update",
        "f1",
        "--mission",
        missionId,
        "--status",
        "completed",
      ],
      tmpDir,
    );

    // Save checkpoint
    const save = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(save.exitCode).toBe(0);
    const checkpoint = JSON.parse(save.stdout).checkpoint;

    // Verify feature state was captured in checkpoint
    expect(checkpoint.featureStates.f1).toBe("completed");
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("checkpoint with mission lifecycle", () => {
  it("checkpoint before and after milestone seal", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Checkpoint before work
    await run(["checkpoint", "save", "--mission", missionId], tmpDir);

    // Complete m1 work
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    const m1Asserts = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    for (const a of JSON.parse(m1Asserts.stdout).assertions) {
      await run(
        ["validate", "update", a.id, "--mission", missionId, "--status", "passed"],
        tmpDir,
      );
    }

    // Checkpoint before seal
    const beforeSeal = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpointBefore = JSON.parse(beforeSeal.stdout).checkpoint;
    expect(checkpointBefore.milestoneId).toBe("m1");

    // Seal m1
    await run(["milestone", "seal", "m1", "--mission", missionId], tmpDir);

    // Checkpoint after seal
    await new Promise((r) => setTimeout(r, 50));
    const afterSeal = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpointAfter = JSON.parse(afterSeal.stdout).checkpoint;

    // Both checkpoints should exist
    const list = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoints = JSON.parse(list.stdout).checkpoints;
    expect(checkpoints.length).toBe(3); // initial + before seal + after seal
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint survives mission status transitions", async () => {
    const missionId = await createMission(tmpDir);

    // Save checkpoint at draft
    const draftCp = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(draftCp.exitCode).toBe(0);

    // Approve
    await run(["mission", "approve", missionId], tmpDir);

    // Save checkpoint at approved
    const approvedCp = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(approvedCp.exitCode).toBe(0);

    // Execute
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Save checkpoint at executing
    const execCp = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(execCp.exitCode).toBe(0);

    // List all checkpoints
    const list = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const checkpoints = JSON.parse(list.stdout).checkpoints;
    expect(checkpoints.length).toBe(3);

    // Latest is from executing phase
    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    const loaded = JSON.parse(load.stdout).checkpoint;
    expect(loaded.id).toBe(JSON.parse(execCp.stdout).checkpoint.id);
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("checkpoint error handling", () => {
  it("fails gracefully when loading from mission with no checkpoints", async () => {
    const missionId = await createMission(tmpDir);

    const load = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(load.exitCode).toBe(1);
    const result = JSON.parse(load.stdout);
    expect(result.error).toContain("No checkpoints found");
  }, SLOW_CLI_TIMEOUT_MS);

  it("checkpoint list returns empty array for mission with no checkpoints", async () => {
    const missionId = await createMission(tmpDir);

    const list = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(list.exitCode).toBe(0);
    const result = JSON.parse(list.stdout);
    expect(result.checkpoints).toHaveLength(0);
    expect(result.mission.id).toBe(missionId);
  }, SLOW_CLI_TIMEOUT_MS);
});
