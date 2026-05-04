import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "../helpers/run-cli";
import { initGitRepo } from "../helpers/run-compiled-cli.js";

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
            agentType: "test-skill",
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
      total: 14,
      passed: 10,
      failed: 0,
      skipped: 4,
    });
    expect(renderCheck.screens.map((screen) => [screen.screen, screen.status])).toEqual([
      ["dashboard", "pass"],
      ["features", "pass"],
      ["dependencies", "skip"],
      ["config", "pass"],
      ["memory", "pass"],
      ["graph", "pass"],
      ["agents", "pass"],
      ["dispatch", "skip"],
      ["events", "pass"],
      ["tasks", "pass"],
      ["timeline", "skip"],
      ["principles", "pass"],
      ["help", "pass"],
      ["autopilot", "skip"],
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
      total: 14,
      passed: 14,
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
      "config",
      "memory",
      "graph",
      "agents",
      "dispatch",
      "events",
      "tasks",
      "timeline",
      "principles",
      "help",
      "autopilot",
    ]) {
      expect(preview.stdout).toContain(`--- ${screen} ---`);
    }
    expect(preview.stdout).toContain("--- rendered 14 screens ---");
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
