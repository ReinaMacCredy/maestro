/**
 * Integration tests for feature CLI commands
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
    description: "A test mission for feature CLI integration",
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
        fulfills: ["assertion-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        skillName: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: ["f1"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        skillName: "test-skill",
        verificationSteps: ["step4"],
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-feature-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("feature CLI commands", () => {
  describe("feature list", () => {
    it("feature list --mission <id> returns features for the mission", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("3 feature(s)");
      expect(stdout).toContain("Feature 1");
      expect(stdout).toContain("Feature 2");
      expect(stdout).toContain("Feature 3");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature list --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "list", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.features).toHaveLength(3);
      expect(result.total).toBe(3);
      expect(result.filtered).toBe(3);
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature list --milestone filters by milestone", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "list", "--mission", missionId, "--milestone", "m1"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("2 feature(s)");
      expect(stdout).toContain("Feature 1");
      expect(stdout).toContain("Feature 2");
      expect(stdout).not.toContain("Feature 3");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature list --status filters by status", async () => {
      const missionId = await createMission(tmpDir);

      // First transition f1 to in_progress
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      const { stdout, exitCode } = await run(
        ["feature", "list", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("1 feature(s)");
      expect(stdout).toContain("f1");
      expect(stdout).not.toContain("f2");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature list combines milestone and status filters", async () => {
      const missionId = await createMission(tmpDir);

      // Transition f1 (in m1) to in_progress
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      const { stdout, exitCode } = await run(
        [
          "feature",
          "list",
          "--mission",
          missionId,
          "--milestone",
          "m1",
          "--status",
          "in_progress",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("1 feature(s)");
      expect(stdout).toContain("f1");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature list errors with hints for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["feature", "list", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
      expect(output).toContain("maestro mission list");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("feature update", () => {
    it("feature update --status transitions feature status", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Feature updated: f1");
      expect(stdout).toContain("Status: in_progress");

      // Verify by listing
      const listResult = await run(
        ["feature", "list", "--mission", missionId, "--json"],
        tmpDir,
      );
      const features = JSON.parse(listResult.stdout).features;
      const f1 = features.find((f: { id: string }) => f.id === "f1");
      expect(f1.status).toBe("in_progress");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update --status rejects illegal transitions", async () => {
      const missionId = await createMission(tmpDir);

      // Try to go directly from pending to completed (illegal)
      const { stdout, stderr, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "completed"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Invalid feature transition");
      expect(output).toContain("Valid transitions from pending");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update allows retry from in_review to pending", async () => {
      const missionId = await createMission(tmpDir);

      // Move through states: pending -> in_progress -> in_review
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_review"],
        tmpDir,
      );

      // Retry: in_review -> pending
      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "pending"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Status: pending");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update allows retry from blocked to pending", async () => {
      const missionId = await createMission(tmpDir);

      // Move through states: pending -> in_progress -> in_review -> blocked
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_review"],
        tmpDir,
      );
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "blocked"],
        tmpDir,
      );

      // Retry: blocked -> pending
      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "pending"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Status: pending");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update with --report attaches worker report", async () => {
      const missionId = await createMission(tmpDir);

      const report = {
        content: "Feature implementation complete",
        timestamp: "2026-03-28T10:00:00.000Z",
        agent: "test-agent",
      };

      const { stdout, exitCode } = await run(
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

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Feature updated: f1");
      expect(stdout).toContain("Report:");
      expect(stdout).toContain("report.json");
      expect(stdout).toContain("2026-03-28T10:00:00.000Z");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update with --report @file reads report from file", async () => {
      const missionId = await createMission(tmpDir);

      const report = {
        content: "File-based report content",
        timestamp: "2026-03-28T12:00:00.000Z",
        agent: "file-agent",
      };
      const reportPath = join(tmpDir, "report.json");
      await writeFile(reportPath, JSON.stringify(report));

      const { stdout, exitCode } = await run(
        [
          "feature",
          "update",
          "f1",
          "--mission",
          missionId,
          "--status",
          "in_progress",
          "--report",
          `@${reportPath}`,
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Feature updated: f1");
      expect(stdout).toContain("Report:");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_progress", "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.feature.id).toBe("f1");
      expect(result.feature.status).toBe("in_progress");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update errors for non-existent feature", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["feature", "update", "nonexistent", "--mission", missionId, "--status", "in_progress"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Feature nonexistent not found");
      expect(output).toContain("maestro feature list");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update errors when no update specified", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("No update specified");
    }, SLOW_CLI_TIMEOUT_MS);

    it("worker report persists after retry update without replacement report", async () => {
      const missionId = await createMission(tmpDir);

      // First attach a report
      const report1 = {
        content: "First implementation attempt",
        timestamp: "2026-03-28T10:00:00.000Z",
        agent: "agent-1",
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
          JSON.stringify(report1),
        ],
        tmpDir,
      );

      // Move to in_review
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in_review"],
        tmpDir,
      );

      // Retry to pending WITHOUT providing a new report
      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "pending"],
        tmpDir,
      );

      expect(exitCode).toBe(0);

      // Verify the report is preserved via JSON output
      const listResult = await run(
        ["feature", "list", "--mission", missionId, "--json"],
        tmpDir,
      );
      const features = JSON.parse(listResult.stdout).features;
      const f1 = features.find((f: { id: string }) => f.id === "f1");

      expect(f1.status).toBe("pending");
      expect(f1.report).toBeDefined();
      expect(f1.report.content).toBe("First implementation attempt");
      expect(f1.report.agent).toBe("agent-1");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("JSON flag positions", () => {
    it("JSON output works from root --json position", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["--json", "feature", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(() => JSON.parse(stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON output works from group --json position", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "--json", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(() => JSON.parse(stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);
  });
});
