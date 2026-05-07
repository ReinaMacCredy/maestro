import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  REPO_ROOT,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

interface RenderCheckResult {
  summary: {
    total: number;
    passed: number;
    failed: number;
    skipped: number;
  };
}

let tmpDir: string;

async function createSkill(baseDir: string, skillName: string): Promise<void> {
  const skillDir = join(baseDir, ".maestro", "skills", skillName);
  await mkdir(skillDir, { recursive: true });
  await writeFile(
    join(skillDir, "SKILL.md"),
    `# ${skillName}\n\nThis is a test skill for ${skillName}.\n`,
  );
}

async function writePipelinePlan(cwd: string): Promise<string> {
  const planPath = join(cwd, "pipeline-plan.json");
  await writeFile(
    planPath,
    JSON.stringify(
      {
        title: "Compiled Pipeline Mission",
        description: "Compiled Maestro CLI workflow from plan through completion",
        milestones: [
          { id: "m1", title: "Foundation", description: "Core workflow", order: 0 },
          { id: "m2", title: "Ship", description: "Validation and release checks", order: 1 },
        ],
        features: [
          {
            id: "f1",
            milestoneId: "m1",
            title: "Bootstrap agent prompt",
            description: "Generate the first agent prompt and complete the setup task",
            agentType: "test-skill",
            verificationSteps: ["Confirm prompt generation", "Confirm setup completed"],
            fulfills: ["assertion-bootstrap-1"],
          },
          {
            id: "f2",
            milestoneId: "m1",
            title: "Deliver core feature",
            description: "Complete the dependent core task",
            agentType: "test-skill",
            verificationSteps: ["Verify core implementation"],
            dependsOn: ["f1"],
            fulfills: ["assertion-core-1", "assertion-core-2"],
          },
          {
            id: "f3",
            milestoneId: "m2",
            title: "Validate release workflow",
            description: "Finish the final validation step",
            agentType: "test-skill",
            verificationSteps: ["Run release validation"],
            fulfills: ["assertion-release-1"],
          },
        ],
      },
      null,
      2,
    ),
  );

  return planPath;
}

async function createMission(cwd: string): Promise<string> {
  const planPath = await writePipelinePlan(cwd);
  const result = await runCompiled(
    ["mission", "create", "--file", planPath, "--json"],
    cwd,
  );

  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout).mission.id as string;
}

async function passAssertionsForMilestone(
  missionId: string,
  milestoneId: string,
  cwd: string,
): Promise<string[]> {
  const showResult = await runCompiled(
    ["validate", "show", "--mission", missionId, "--milestone", milestoneId, "--json"],
    cwd,
  );
  expect(showResult.exitCode).toBe(0);

  const assertions = JSON.parse(showResult.stdout).assertions as Array<{ id: string }>;
  const updatedIds: string[] = [];

  for (const assertion of assertions) {
    const updateResult = await runCompiled(
      [
        "validate",
        "update",
        assertion.id,
        "--mission",
        missionId,
        "--result",
        "passed",
        "--evidence",
        `Compiled pipeline verification passed for ${assertion.id}`,
        "--json",
      ],
      cwd,
    );
    expect(updateResult.exitCode).toBe(0);
    updatedIds.push(assertion.id);
  }

  return updatedIds;
}

beforeAll(async () => {
  await buildCompiledCli();

  const versionResult = await runCompiled(["--version"], REPO_ROOT);
  expect(versionResult.exitCode).toBe(0);
  expect(versionResult.stdout).toContain("-g");
}, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-compiled-pipeline-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("compiled CLI pipeline E2E", () => {
  it("runs a full Maestro workflow from plan file through completion", async () => {
    const missionId = await createMission(tmpDir);

    const promptResult = await runCompiled(
      ["feature", "prompt", "f1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(promptResult.exitCode).toBe(0);
    const promptData = JSON.parse(promptResult.stdout);
    expect(promptData.featureId).toBe("f1");
    expect(promptData.agentType).toBe("test-skill");
    expect(promptData.prompt).toContain("# Agent Assignment: Bootstrap agent prompt");
    const promptPath = join(
      tmpDir,
      ".maestro",
      "missions",
      missionId,
      "agents",
      "f1",
      "prompt.md",
    );
    expect(await readFile(promptPath, "utf8")).toContain("## Skill Instructions");

    const approveResult = await runCompiled(
      ["mission", "approve", missionId, "--json"],
      tmpDir,
    );
    expect(approveResult.exitCode).toBe(0);
    expect(JSON.parse(approveResult.stdout).status).toBe("approved");

    const f1InProgress = await runCompiled(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"],
      tmpDir,
    );
    expect(f1InProgress.exitCode).toBe(0);
    const f1InProgressData = JSON.parse(f1InProgress.stdout);
    expect(f1InProgressData.feature.status).toBe("in-progress");
    expect(f1InProgressData.missionAutoStarted).toBe(true);

    const executingMissionShow = await runCompiled(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );
    expect(executingMissionShow.exitCode).toBe(0);
    expect(JSON.parse(executingMissionShow.stdout).effectiveMissionStatus).toBe("executing");

    const report = JSON.stringify({
      content: "Bootstrap agent prompt completed successfully",
      timestamp: new Date().toISOString(),
      agent: "compiled-e2e-agent",
    });

    for (const status of ["review", "done"] as const) {
      const args = [
        "feature",
        "update",
        "f1",
        "--mission",
        missionId,
        "--status",
        status,
        "--json",
      ];
      if (status === "done") {
        args.push("--report", report);
      }
      const result = await runCompiled(args, tmpDir);
      expect(result.exitCode).toBe(0);
      expect(JSON.parse(result.stdout).feature.status).toBe(status);
    }

    for (const status of ["assigned", "in-progress", "review", "done"] as const) {
      const result = await runCompiled(
        ["feature", "update", "f2", "--mission", missionId, "--status", status, "--json"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      expect(JSON.parse(result.stdout).feature.status).toBe(status);
    }

    const m1Assertions = await passAssertionsForMilestone(missionId, "m1", tmpDir);
    expect(m1Assertions).toHaveLength(3);

    const validatingResult = await runCompiled(
      ["mission", "update", missionId, "--status", "validating", "--json"],
      tmpDir,
    );
    expect(validatingResult.exitCode).toBe(0);
    expect(JSON.parse(validatingResult.stdout).status).toBe("validating");

    const sealM1 = await runCompiled(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealM1.exitCode).toBe(0);
    expect(JSON.parse(sealM1.stdout).sealed).toBe(true);

    const checkpointSave = await runCompiled(
      ["checkpoint", "save", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(checkpointSave.exitCode).toBe(0);
    const checkpoint = JSON.parse(checkpointSave.stdout).checkpoint;
    expect(checkpoint.featureStatuses).toMatchObject({
      f1: "done",
      f2: "done",
      f3: "pending",
    });

    const mutateF3 = await runCompiled(
      ["feature", "update", "f3", "--mission", missionId, "--status", "in-progress", "--json"],
      tmpDir,
    );
    expect(mutateF3.exitCode).toBe(0);
    expect(JSON.parse(mutateF3.stdout).feature.status).toBe("in-progress");

    const checkpointLoad = await runCompiled(
      ["checkpoint", "load", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(checkpointLoad.exitCode).toBe(0);
    const checkpointLoadData = JSON.parse(checkpointLoad.stdout);
    expect(checkpointLoadData.checkpoint.id).toBe(checkpoint.id);
    expect(checkpointLoadData.restored).toEqual({
      featureCount: 1,
      assertionCount: 0,
    });

    const featureListAfterLoad = await runCompiled(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(featureListAfterLoad.exitCode).toBe(0);
    const featureStatusesAfterLoad = new Map(
      (JSON.parse(featureListAfterLoad.stdout).features as Array<{ id: string; status: string }>).map(
        (feature) => [feature.id, feature.status],
      ),
    );
    expect(featureStatusesAfterLoad.get("f3")).toBe("pending");

    const f3PromptResult = await runCompiled(
      ["feature", "prompt", "f3", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(f3PromptResult.exitCode).toBe(0);
    expect(JSON.parse(f3PromptResult.stdout).prompt).toContain("Validate release workflow");

    for (const status of ["assigned", "in-progress", "review", "done"] as const) {
      const result = await runCompiled(
        ["feature", "update", "f3", "--mission", missionId, "--status", status, "--json"],
        tmpDir,
      );
      expect(result.exitCode).toBe(0);
      expect(JSON.parse(result.stdout).feature.status).toBe(status);
    }

    const m2Assertions = await passAssertionsForMilestone(missionId, "m2", tmpDir);
    expect(m2Assertions).toHaveLength(1);

    const sealM2 = await runCompiled(
      ["milestone", "seal", "m2", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealM2.exitCode).toBe(0);
    expect(JSON.parse(sealM2.stdout).sealed).toBe(true);

    const finalValidatingResult = await runCompiled(
      ["mission", "update", missionId, "--status", "validating", "--json"],
      tmpDir,
    );
    expect(finalValidatingResult.exitCode).toBe(0);
    expect(JSON.parse(finalValidatingResult.stdout).status).toBe("validating");

    const completeResult = await runCompiled(
      ["mission", "update", missionId, "--status", "completed", "--json"],
      tmpDir,
    );
    expect(completeResult.exitCode).toBe(0);
    expect(JSON.parse(completeResult.stdout).status).toBe("completed");

    const missionShow = await runCompiled(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );
    expect(missionShow.exitCode).toBe(0);
    const missionData = JSON.parse(missionShow.stdout);
    expect(missionData.mission.status).toBe("completed");
    expect(missionData.summary.totalFeatures).toBe(3);

    const featureList = await runCompiled(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(featureList.exitCode).toBe(0);
    const featureStatuses = new Map(
      (JSON.parse(featureList.stdout).features as Array<{ id: string; status: string }>).map(
        (feature) => [feature.id, feature.status],
      ),
    );
    expect(featureStatuses.get("f1")).toBe("done");
    expect(featureStatuses.get("f2")).toBe("done");
    expect(featureStatuses.get("f3")).toBe("done");

    const renderCheck = await runCompiled(
      ["mission-control", "--mission", missionId, "--render-check", "--size", "120x40"],
      tmpDir,
    );
    expect(renderCheck.exitCode).toBe(0);
    const renderCheckData = JSON.parse(renderCheck.stdout) as RenderCheckResult;
      expect(renderCheckData.summary).toEqual({
        total: 14,
        passed: 14,
        failed: 0,
        skipped: 0,
      });
  }, SLOW_CLI_TIMEOUT_MS);
});
