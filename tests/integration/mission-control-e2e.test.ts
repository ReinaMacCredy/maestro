import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "../helpers/run-cli";

interface RenderCheckScreen {
  screen: string;
  status: "pass" | "fail" | "skip";
  size: string;
  warnings: string[];
}

interface RenderCheckResult {
  screens: RenderCheckScreen[];
  summary: {
    total: number;
    passed: number;
    failed: number;
    skipped: number;
  };
}

interface FeatureListResult {
  features: Array<{
    id: string;
    status: string;
  }>;
}

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 20_000;

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
}

async function createSkill(baseDir: string, skillName: string): Promise<void> {
  const skillDir = join(baseDir, ".maestro", "skills", skillName);
  await mkdir(skillDir, { recursive: true });
  await writeFile(
    join(skillDir, "SKILL.md"),
    `# ${skillName}\n\nTest skill fixture for mission-control integration coverage.\n`,
  );
}

async function createMission(cwd: string): Promise<string> {
  const planPath = join(cwd, "plan.json");
  await writeFile(
    planPath,
    JSON.stringify(
      {
        title: "Mission Control E2E Test",
        description: "Read-only mission control smoke coverage",
        milestones: [
          { id: "m1", title: "Milestone 1", description: "First", order: 0 },
        ],
        features: [
          {
            id: "f1",
            milestoneId: "m1",
            title: "Feature 1",
            description: "First feature",
            workerType: "test-skill",
            verificationSteps: ["check it"],
          },
        ],
      },
      null,
      2,
    ),
  );

  const result = await runCli(
    ["mission", "create", "--file", planPath, "--json"],
    cwd,
  );
  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout).mission.id as string;
}

function parseRenderCheck(stdout: string): RenderCheckResult {
  return JSON.parse(stdout) as RenderCheckResult;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-control-e2e-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("mission-control E2E", () => {
  it("renders the home-mode check matrix without failures", async () => {
    const result = await runCli(
      ["mission-control", "--render-check", "--size", "120x40"],
      tmpDir,
    );

    expect(result.exitCode).toBe(0);

    const renderCheck = parseRenderCheck(result.stdout);
    expect(renderCheck.summary).toEqual({
      total: 7,
      passed: 5,
      failed: 0,
      skipped: 2,
    });
    expect(renderCheck.screens.map((screen) => [screen.screen, screen.status])).toEqual([
      ["dashboard", "pass"],
      ["features", "pass"],
      ["dependencies", "skip"],
      ["handoffs", "skip"],
      ["config", "pass"],
      ["memory", "pass"],
      ["graph", "pass"],
    ]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("renders all mission preview screens and keeps feature state unchanged", async () => {
    const missionId = await createMission(tmpDir);

    const approve = await runCli(["mission", "approve", missionId, "--json"], tmpDir);
    expect(approve.exitCode).toBe(0);

    const assign = await runCli(
      ["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"],
      tmpDir,
    );
    expect(assign.exitCode).toBe(0);

    const renderCheckResult = await runCli(
      ["mission-control", "--mission", missionId, "--render-check", "--size", "120x40"],
      tmpDir,
    );
    expect(renderCheckResult.exitCode).toBe(0);

    const renderCheck = parseRenderCheck(renderCheckResult.stdout);
    expect(renderCheck.summary).toEqual({
      total: 7,
      passed: 7,
      failed: 0,
      skipped: 0,
    });

    const preview = await runCli(
      [
        "mission-control",
        "--mission",
        missionId,
        "--preview",
        "all",
        "--size",
        "120x40",
        "--format",
        "plain",
      ],
      tmpDir,
    );

    expect(preview.exitCode).toBe(0);
    for (const screen of [
      "dashboard",
      "features",
      "dependencies",
      "handoffs",
      "config",
      "memory",
      "graph",
    ]) {
      expect(preview.stdout).toContain(`--- ${screen} ---`);
    }
    expect(preview.stdout).toContain("--- rendered 7 screens ---");
    expect(preview.stdout).toContain("Mission Control E2E Test");
    expect(preview.stdout).toContain("f1 Feature 1");
    expect(preview.stdout).not.toContain("undefined");
    expect(preview.stdout).not.toContain("NaN");

    const featureListResult = await runCli(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(featureListResult.exitCode).toBe(0);

    const featureList = JSON.parse(featureListResult.stdout) as FeatureListResult;
    expect(featureList.features.find((feature) => feature.id === "f1")?.status).toBe(
      "assigned",
    );
  }, SLOW_CLI_TIMEOUT_MS);
});
