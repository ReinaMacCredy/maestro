/**
 * Integration tests for multi-milestone progression
 * Tests: progression through multiple milestones, state isolation, cross-milestone status
 * Fulfills: VAL-CROSS-001, VAL-MILESTONE-001, VAL-MILESTONE-002
 */
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, writeFile, rm, mkdir } from "node:fs/promises";
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

function createMultiMilestonePlan(): object {
  return {
    title: "Multi-Milestone Mission",
    description: "Mission with three milestones for testing progression",
    milestones: [
      { id: "m1", title: "Foundation", description: "Base layer", order: 0 },
      { id: "m2", title: "Core Features", description: "Main work", order: 1 },
      { id: "m3", title: "Polish", description: "Final touches", order: 2 },
    ],
    features: [
      // Milestone 1 features
      {
        id: "f1-m1",
        milestoneId: "m1",
        title: "M1 Feature 1",
        description: "Foundation work 1",
        workerType: "test-skill",
        verificationSteps: ["Step 1"],
        fulfills: ["assert-m1-1"],
      },
      {
        id: "f2-m1",
        milestoneId: "m1",
        title: "M1 Feature 2",
        description: "Foundation work 2",
        workerType: "test-skill",
        verificationSteps: ["Step 2"],
        fulfills: ["assert-m1-2"], // Add fulfills for assertions
      },
      // Milestone 2 features
      {
        id: "f3-m2",
        milestoneId: "m2",
        title: "M2 Feature 1",
        description: "Core work 1",
        workerType: "test-skill",
        verificationSteps: ["Step 3"],
        fulfills: ["assert-m2-1"],
      },
      {
        id: "f4-m2",
        milestoneId: "m2",
        title: "M2 Feature 2",
        description: "Core work 2",
        workerType: "test-skill",
        verificationSteps: ["Step 4"],
        fulfills: ["assert-m2-2"],
      },
      // Milestone 3 features
      {
        id: "f5-m3",
        milestoneId: "m3",
        title: "M3 Feature 1",
        description: "Polish work 1",
        workerType: "test-skill",
        verificationSteps: ["Step 5"],
      },
    ],
  };
}

async function createMission(cwd: string): Promise<string> {
  const plan = createMultiMilestonePlan();
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-multi-milestone-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("multi-milestone progression", () => {
  it("milestone list shows all milestones with correct progress", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    const listResult = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(listResult.exitCode).toBe(0);
    const data = JSON.parse(listResult.stdout);

    expect(data.mission).toBeDefined();
    expect(data.mission.id).toBe(missionId);
    expect(data.milestones).toHaveLength(3);

    // All milestones should show 0% completion initially
    for (const m of data.milestones) {
      expect(m.featureCompletionPct).toBe(0);
      expect(m.completedFeatures).toBe(0);
    }
  }, SLOW_CLI_TIMEOUT_MS);

  it("progresses through milestones sequentially", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Complete m1 features with verification - need to go through in_progress and in_review
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    const f1Result = await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "done", "--json"],
      tmpDir,
    );
    expect(f1Result.exitCode).toBe(0);
    expect(JSON.parse(f1Result.stdout).feature.status).toBe("done");

    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    const f2Result = await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "done", "--json"],
      tmpDir,
    );
    expect(f2Result.exitCode).toBe(0);
    expect(JSON.parse(f2Result.stdout).feature.status).toBe("done");

    // Pass all m1 assertions
    const m1Asserts = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    for (const assertion of JSON.parse(m1Asserts.stdout).assertions) {
      await run(
        ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    // Check m1 progress
    const m1Status = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(m1Status.exitCode).toBe(0);
    const m1Data = JSON.parse(m1Status.stdout);
    // Milestone status returns { mission, milestone, progress }
    expect(m1Data.progress.completedFeatures).toBe(2);
    expect(m1Data.progress.featureCount).toBe(2);
    expect(m1Data.progress.featureCompletionPct).toBe(100);

    // Seal m1
    const sealM1 = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealM1.exitCode).toBe(0);

    // Complete m2 features
    await run(
      ["feature", "update", "f3-m2", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f3-m2", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f3-m2", "--mission", missionId, "--status", "done"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f4-m2", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f4-m2", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f4-m2", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    // Pass all m2 assertions
    const m2Asserts = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m2", "--json"],
      tmpDir,
    );
    for (const assertion of JSON.parse(m2Asserts.stdout).assertions) {
      await run(
        ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    // Seal m2
    const sealM2 = await run(
      ["milestone", "seal", "m2", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealM2.exitCode).toBe(0);

    // Complete m3 features
    await run(
      ["feature", "update", "f5-m3", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f5-m3", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f5-m3", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    // Check overall milestone list
    const finalList = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const finalData = JSON.parse(finalList.stdout);

    const m1Final = finalData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m1");
    const m2Final = finalData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m2");
    const m3Final = finalData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m3");

    expect(m1Final.status).toBe("sealed");
    expect(m2Final.status).toBe("sealed");
    expect(m3Final.completedFeatures).toBe(1);
  }, SLOW_CLI_TIMEOUT_MS);

  it("shows correct feature counts per milestone", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Check m1 status
    const m1Result = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(m1Result.exitCode).toBe(0);
    const m1Data = JSON.parse(m1Result.stdout);
    expect(m1Data.progress.featureCount).toBe(2);

    // Check m2 status
    const m2Result = await run(
      ["milestone", "status", "m2", "--mission", missionId, "--json"],
      tmpDir,
    );
    const m2Data = JSON.parse(m2Result.stdout);
    expect(m2Data.progress.featureCount).toBe(2);

    // Check m3 status
    const m3Result = await run(
      ["milestone", "status", "m3", "--mission", missionId, "--json"],
      tmpDir,
    );
    const m3Data = JSON.parse(m3Result.stdout);
    expect(m3Data.progress.featureCount).toBe(1);
  }, SLOW_CLI_TIMEOUT_MS);

  it("feature list filters correctly by milestone", async () => {
    const missionId = await createMission(tmpDir);

    // List all features
    const allFeatures = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(JSON.parse(allFeatures.stdout).features).toHaveLength(5);

    // List m1 features only
    const m1Features = await run(
      ["feature", "list", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    const m1Data = JSON.parse(m1Features.stdout);
    expect(m1Data.features).toHaveLength(2);
    expect(m1Data.filtered).toBe(2);
    expect(m1Data.total).toBe(5);
    expect(m1Data.features.every((f: { milestoneId: string }) => f.milestoneId === "m1")).toBe(true);

    // List m2 features only
    const m2Features = await run(
      ["feature", "list", "--mission", missionId, "--milestone", "m2", "--json"],
      tmpDir,
    );
    expect(JSON.parse(m2Features.stdout).features).toHaveLength(2);

    // List m3 features only
    const m3Features = await run(
      ["feature", "list", "--mission", missionId, "--milestone", "m3", "--json"],
      tmpDir,
    );
    expect(JSON.parse(m3Features.stdout).features).toHaveLength(1);
  }, SLOW_CLI_TIMEOUT_MS);

  it("validate show filters by milestone", async () => {
    const missionId = await createMission(tmpDir);

    // All assertions
    const all = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    const allCount = JSON.parse(all.stdout).assertions.length;
    expect(allCount).toBeGreaterThan(0);

    // m1 assertions
    const m1 = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    const m1Count = JSON.parse(m1.stdout).assertions.length;

    // m2 assertions
    const m2 = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m2", "--json"],
      tmpDir,
    );
    const m2Count = JSON.parse(m2.stdout).assertions.length;

    expect(m1Count + m2Count).toBe(allCount);
  }, SLOW_CLI_TIMEOUT_MS);

  it("milestone list shows progress percentages at various completion stages", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Initial state - 0%
    const initial = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const initialData = JSON.parse(initial.stdout);
    expect(initialData.milestones[0].featureCompletionPct).toBe(0);

    // Complete 1 of 2 features in m1
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    const halfDone = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const halfData = JSON.parse(halfDone.stdout);
    const m1Half = halfData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m1");
    expect(m1Half.completedFeatures).toBe(1);
    expect(m1Half.featureCount).toBe(2);

    // Complete all m1 features
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    const allDone = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const allData = JSON.parse(allDone.stdout);
    const m1All = allData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m1");
    expect(m1All.completedFeatures).toBe(2);
    expect(m1All.featureCompletionPct).toBe(100);
  }, SLOW_CLI_TIMEOUT_MS);

  it("handles mixed milestone states during mission execution", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);
    await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    // Complete m1
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    const m1Asserts = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    for (const assertion of JSON.parse(m1Asserts.stdout).assertions) {
      await run(
        ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    await run(["milestone", "seal", "m1", "--mission", missionId], tmpDir);

    // Start m2 work
    await run(
      ["feature", "update", "f3-m2", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );

    // m3 still pending

    // Check mixed state in milestone list
    const listResult = await run(
      ["milestone", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const listData = JSON.parse(listResult.stdout);

    const m1 = listData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m1");
    const m2 = listData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m2");
    const m3 = listData.milestones.find((m: { milestoneId: string }) => m.milestoneId === "m3");

    expect(m1.status).toBe("sealed");
    expect(m1.featureCompletionPct).toBe(100);

    expect(m2.status).toBe("executing"); // m2 is the current active milestone
    expect(m2.completedFeatures).toBe(0);

    expect(m3.status).toBe("pending"); // m3 comes after the current milestone
    expect(m3.completedFeatures).toBe(0);
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("milestone completion tracking", () => {
  it("tracks completion percentage through various stages", async () => {
    const missionId = await createMission(tmpDir);

    // Get m1 assertions
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    const assertionIds = JSON.parse(assertions.stdout).assertions.map(
      (a: { id: string }) => a.id,
    );

    // No assertions passed yet
    const initial = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(JSON.parse(initial.stdout).progress.assertionCompletionPct).toBe(0);

    // Pass half
    if (assertionIds.length >= 1) {
      await run(
        ["validate", "update", assertionIds[0], "--mission", missionId, "--result", "passed"],
        tmpDir,
      );

      const half = await run(
        ["milestone", "status", "m1", "--mission", missionId, "--json"],
        tmpDir,
      );
      const halfData = JSON.parse(half.stdout);
      expect(halfData.progress.passedAssertions).toBe(1);
    }

    // Pass all
    for (const id of assertionIds) {
      await run(
        ["validate", "update", id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    const all = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    const allData = JSON.parse(all.stdout);
    expect(allData.progress.assertionCompletionPct).toBe(100);
    expect(allData.progress.passedAssertions).toBe(assertionIds.length);
  }, SLOW_CLI_TIMEOUT_MS);

  it("correctly identifies waived assertions in milestone status", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Complete features
    await run(
      ["feature", "update", "f1-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2-m1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    // Get assertions
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    const assertionList = JSON.parse(assertions.stdout).assertions;
    expect(assertionList.length).toBeGreaterThanOrEqual(1);

    // Pass all but one, waive the last one
    for (let i = 0; i < assertionList.length - 1; i++) {
      await run(
        ["validate", "update", assertionList[i].id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    // Waive the last assertion
    const lastId = assertionList[assertionList.length - 1].id;
    await run(
      [
        "validate",
        "update",
        lastId,
        "--mission",
        missionId,
        "--result",
        "waived",
        "--reason",
        "Not applicable for this release",
      ],
      tmpDir,
    );

    // Check milestone status
    const status = await run(
      ["milestone", "status", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    const statusData = JSON.parse(status.stdout);
    expect(statusData.progress.waivedAssertions).toBe(1);

    // Seal should succeed
    const seal = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(seal.exitCode).toBe(0);
    const sealData = JSON.parse(seal.stdout);
    expect(sealData.progress.waivedAssertionIds).toHaveLength(1);
    expect(sealData.progress.waivedAssertionIds[0]).toBe(lastId);
  }, SLOW_CLI_TIMEOUT_MS);
});
