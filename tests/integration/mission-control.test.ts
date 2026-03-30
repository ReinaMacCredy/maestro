/**
 * Integration tests for mission-control command
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
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
}

function createSamplePlan(): object {
  return {
    title: "MC Test Mission",
    description: "A mission for mission-control integration tests",
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
        fulfills: ["a-f1-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        workerType: "test-skill",
        verificationSteps: ["verify it"],
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mc-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("mission-control CLI", () => {
  it("--json returns valid snapshot with expected fields", async () => {
    const missionId = await createMission(tmpDir);

    const { stdout, exitCode } = await run(
      ["mission-control", "--mission", missionId, "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const snapshot = JSON.parse(stdout);
    expect(snapshot.missionId).toBe(missionId);
    expect(snapshot.missionTitle).toBe("MC Test Mission");
    expect(snapshot.featureProgress).toBeDefined();
    expect(snapshot.featureProgress.total).toBe(2);
    expect(snapshot.features).toBeDefined();
    expect(Array.isArray(snapshot.features)).toBe(true);
    expect(snapshot.features.length).toBe(2);
    expect(snapshot.progressLog).toBeDefined();
    expect(snapshot.milestones).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("--once returns non-empty text containing mission title", async () => {
    const missionId = await createMission(tmpDir);

    const { stdout, exitCode } = await run(
      ["mission-control", "--mission", missionId, "--once"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout.length).toBeGreaterThan(0);
    expect(stdout).toContain("Mission Control");
    expect(stdout).toContain("Features");
  }, SLOW_CLI_TIMEOUT_MS);

  it("--json auto-selects mission when --mission omitted", async () => {
    const missionId = await createMission(tmpDir);

    const { stdout, exitCode } = await run(
      ["mission-control", "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const snapshot = JSON.parse(stdout);
    expect(snapshot.missionId).toBe(missionId);
  }, SLOW_CLI_TIMEOUT_MS);

  it("errors for non-existent mission", async () => {
    const { stdout, stderr, exitCode } = await run(
      ["mission-control", "--mission", "2026-03-30-nonexistent", "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("not found");
  }, SLOW_CLI_TIMEOUT_MS);

  it("errors when no missions exist", async () => {
    const { stdout, stderr, exitCode } = await run(
      ["mission-control", "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("No missions found");
  }, SLOW_CLI_TIMEOUT_MS);
});
