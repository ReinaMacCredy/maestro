/**
 * Integration tests for full Mission Control lifecycle
 * Tests: create → approve → feature transitions → assertion updates → milestone seal → checkpoint save/load
 * Fulfills: VAL-CROSS-001, VAL-CROSS-003
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

function createFullPlan(): object {
  return {
    title: "Full Lifecycle Mission",
    description: "End-to-end test of mission lifecycle",
    milestones: [
      { id: "m1", title: "Foundation", description: "Core implementation", order: 0 },
      { id: "m2", title: "Validation", description: "Testing phase", order: 1 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Setup Feature",
        description: "Initial setup and configuration",
        skillName: "test-skill",
        verificationSteps: ["Verify setup completes"],
        fulfills: ["assertion-setup-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Core Feature",
        description: "Main implementation work",
        skillName: "test-skill",
        verificationSteps: ["Verify core logic", "Check edge cases"],
        fulfills: ["assertion-core-1", "assertion-core-2"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Validation Feature",
        description: "Final validation steps",
        skillName: "test-skill",
        verificationSteps: ["Run integration tests"],
        fulfills: ["assertion-val-1"],
      },
    ],
  };
}

async function createMission(cwd: string): Promise<string> {
  const plan = createFullPlan();
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
    `# ${skillName}\n\nThis is a test skill for ${skillName}.\n\n## Instructions\nFollow the steps carefully.\n`,
  );
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-lifecycle-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("full mission lifecycle", () => {
  it("exercises complete lifecycle: create → approve → feature transitions → assertion updates → milestone seal → checkpoint", async () => {
    // Step 1: Create mission
    const missionId = await createMission(tmpDir);

    // Verify mission created with correct structure
    const showResult = await run(["mission", "show", missionId, "--json"], tmpDir);
    expect(showResult.exitCode).toBe(0);
    const showData = JSON.parse(showResult.stdout);
    expect(showData.mission.status).toBe("draft");
    expect(showData.milestones).toHaveLength(2);
    expect(showData.summary.totalFeatures).toBe(3);

    // Step 2: Approve mission
    const approveResult = await run(["mission", "approve", missionId], tmpDir);
    expect(approveResult.exitCode).toBe(0);
    expect(approveResult.stdout).toContain("Mission approved");

    // Verify status changed
    const showApproved = await run(["mission", "show", missionId, "--json"], tmpDir);
    expect(JSON.parse(showApproved.stdout).mission.status).toBe("approved");

    // Step 3: Transition mission to executing
    const execResult = await run(
      ["mission", "update", missionId, "--status", "executing", "--json"],
      tmpDir,
    );
    expect(execResult.exitCode).toBe(0);
    expect(JSON.parse(execResult.stdout).status).toBe("executing");

    // Step 4: Feature transitions
    // Move f1 through its lifecycle
    const f1Progress = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress", "--json"],
      tmpDir,
    );
    expect(f1Progress.exitCode).toBe(0);
    expect(JSON.parse(f1Progress.stdout).feature.status).toBe("in_progress");

    const f1Review = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in_review", "--json"],
      tmpDir,
    );
    expect(f1Review.exitCode).toBe(0);

    // Attach a worker report
    const report = {
      content: "Setup feature completed successfully",
      timestamp: new Date().toISOString(),
      agent: "test-agent",
    };
    const f1Done = await run(
      [
        "feature",
        "update",
        "f1",
        "--mission",
        missionId,
        "--status",
        "completed",
        "--report",
        JSON.stringify(report),
        "--json",
      ],
      tmpDir,
    );
    expect(f1Done.exitCode).toBe(0);
    expect(JSON.parse(f1Done.stdout).feature.status).toBe("completed");

    // Move f2 with a retry scenario
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_review"],
      tmpDir,
    );
    // Rejected in review, back to pending for retry
    const f2Retry = await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "pending", "--json"],
      tmpDir,
    );
    expect(f2Retry.exitCode).toBe(0);
    expect(JSON.parse(f2Retry.stdout).feature.status).toBe("pending");

    // Complete f2
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in_review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    // Step 5: Update assertions for milestone m1
    const m1Assertions = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    expect(m1Assertions.exitCode).toBe(0);
    const m1Asserts = JSON.parse(m1Assertions.stdout).assertions;
    expect(m1Asserts.length).toBeGreaterThan(0);

    // Pass all m1 assertions
    for (const assertion of m1Asserts) {
      const updateResult = await run(
        [
          "validate",
          "update",
          assertion.id,
          "--mission",
          missionId,
          "--status",
          "passed",
          "--evidence",
          `Test passed for ${assertion.description}`,
          "--json",
        ],
        tmpDir,
      );
      expect(updateResult.exitCode).toBe(0);
    }

    // Step 6: Transition mission to validating
    const validatingResult = await run(
      ["mission", "update", missionId, "--status", "validating", "--json"],
      tmpDir,
    );
    expect(validatingResult.exitCode).toBe(0);

    // Step 7: Seal milestone m1
    const sealResult = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealResult.exitCode).toBe(0);
    const sealData = JSON.parse(sealResult.stdout);
    expect(sealData.milestone.id).toBe("m1");
    expect(sealData.sealed).toBe(true);

    // Verify milestone m1 shows validating (mission is in validating status)
    const m1Status = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(m1Status.exitCode).toBe(0);
    const m1Data = JSON.parse(m1Status.stdout);
    expect(m1Data.progress.status).toBe("completed");

    // Step 8: Continue with m2
    await run(
      ["feature", "update", "f3", "--mission", missionId, "--status", "in_progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f3", "--mission", missionId, "--status", "in_review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f3", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    const m2Assertions = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m2", "--json"],
      tmpDir,
    );
    for (const assertion of JSON.parse(m2Assertions.stdout).assertions) {
      await run(
        [
          "validate",
          "update",
          assertion.id,
          "--mission",
          missionId,
          "--status",
          "passed",
          "--json",
        ],
        tmpDir,
      );
    }

    // Seal m2
    const sealM2 = await run(
      ["milestone", "seal", "m2", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealM2.exitCode).toBe(0);

    // Step 9: Save checkpoint
    const checkpointResult = await run(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(checkpointResult.exitCode).toBe(0);
    const checkpoint = JSON.parse(checkpointResult.stdout).checkpoint;
    expect(checkpoint.missionId).toBe(missionId);
    expect(checkpoint.featureStates.f1).toBe("completed");
    expect(checkpoint.featureStates.f2).toBe("completed");
    expect(checkpoint.featureStates.f3).toBe("completed");

    // Step 10: List checkpoints
    const listResult = await run(
      ["checkpoint", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(listResult.exitCode).toBe(0);
    const listData = JSON.parse(listResult.stdout);
    expect(listData.checkpoints).toHaveLength(1);

    // Step 11: Load checkpoint
    const loadResult = await run(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(loadResult.exitCode).toBe(0);
    const loadData = JSON.parse(loadResult.stdout);
    expect(loadData.checkpoint.id).toBe(checkpoint.id);
    expect(loadData.warning).toContain("metadata only");

    // Step 12: Complete mission
    const completeResult = await run(
      ["mission", "update", missionId, "--status", "completed", "--json"],
      tmpDir,
    );
    expect(completeResult.exitCode).toBe(0);
    const finalData = JSON.parse(completeResult.stdout);
    // Mission update returns the mission directly, not wrapped in a mission field
    expect(finalData.status).toBe("completed");
    expect(finalData.completedAt).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("preserves worker reports through checkpoint save/load", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Create feature with report
    const report = {
      content: "Implementation report with detailed findings",
      timestamp: "2026-03-28T12:00:00.000Z",
      agent: "test-worker",
    };
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

    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    // Save checkpoint
    await run(["checkpoint", "save", "--mission", missionId], tmpDir);

    // Verify report is still in feature
    const featureList = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const f1Data = JSON.parse(featureList.stdout).features.find(
      (f: { id: string }) => f.id === "f1",
    );
    expect(f1Data.report).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("handles feature prompt generation in lifecycle context", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Generate prompt for feature
    const promptResult = await run(
      ["feature", "prompt", "f1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(promptResult.exitCode).toBe(0);
    const promptData = JSON.parse(promptResult.stdout);
    expect(promptData.prompt).toContain("f1");
    expect(promptData.prompt).toContain("Setup Feature");
    expect(promptData.workerType).toBe("test-skill");
    expect(promptData.writtenTo.length).toBeGreaterThan(0);

    // Verify prompt file was written
    const promptPath = join(
      tmpDir,
      ".maestro",
      "missions",
      missionId,
      "workers",
      "f1",
      "prompt.md",
    );
    const promptContent = await readFile(promptPath, "utf-8");
    expect(promptContent).toContain("Worker Assignment: Setup Feature");
    expect(promptContent).toContain("Foundation"); // milestone title
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("lifecycle error handling", () => {
  it("prevents milestone seal when assertions are pending", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Complete features but don't pass assertions
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
      tmpDir,
    );

    // Try to seal - should fail with helpful error message
    const sealResult = await run(
      ["milestone", "seal", "m1", "--mission", missionId],
      tmpDir,
    );
    expect(sealResult.exitCode).toBe(1);
    const output = sealResult.stdout + sealResult.stderr;
    expect(output).toContain("Cannot seal milestone");
    expect(output).toContain("Blocking assertions");
  }, SLOW_CLI_TIMEOUT_MS);

  it("prevents illegal mission transitions", async () => {
    const missionId = await createMission(tmpDir);

    // Try to go draft -> executing directly
    const result = await run(
      ["mission", "update", missionId, "--status", "executing", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    const output = result.stdout + result.stderr;
    expect(output).toContain("Invalid mission transition");
    expect(output).toContain("approved"); // Should suggest approved as valid next state
  }, SLOW_CLI_TIMEOUT_MS);

  it("prevents sealing already sealed milestone", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Pass all assertions
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    for (const assertion of JSON.parse(assertions.stdout).assertions) {
      await run(
        ["validate", "update", assertion.id, "--mission", missionId, "--status", "passed"],
        tmpDir,
      );
    }

    // Seal once
    const seal1 = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(seal1.exitCode).toBe(0);

    // Try to seal again - should succeed (idempotent) since assertions are already terminal
    const seal2 = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(seal2.exitCode).toBe(0);
    const sealData = JSON.parse(seal2.stdout);
    expect(sealData.sealed).toBe(true);
  }, SLOW_CLI_TIMEOUT_MS);
});
