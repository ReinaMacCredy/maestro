/**
 * Integration tests for milestone CLI commands
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
    description: "A test mission for milestone CLI integration",
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
        workerType: "test-skill",
        verificationSteps: ["step1", "step2"],
        fulfills: ["assertion-f1-1", "assertion-f1-2"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        workerType: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: ["f1"],
        fulfills: ["assertion-f2-1"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        workerType: "test-skill",
        verificationSteps: ["step4"],
        fulfills: ["assertion-f3-1"],
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-milestone-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("milestone CLI commands", () => {
  describe("milestone list", () => {
    it("milestone list --mission <id> returns milestones with progress", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["milestone", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Test Mission");
      expect(stdout).toContain("m1");
      expect(stdout).toContain("m2");
      expect(stdout).toContain("Milestone 1");
      expect(stdout).toContain("Milestone 2");
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone list --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["milestone", "list", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.mission).toBeDefined();
      expect(result.mission.id).toBe(missionId);
      expect(result.milestones).toBeDefined();
      expect(Array.isArray(result.milestones)).toBe(true);
      expect(result.milestones).toHaveLength(2);
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone list reports feature and assertion counts", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["milestone", "list", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      // m1 has 2 features, m2 has 1 feature
      expect(stdout).toContain("Features: 0/2");
      expect(stdout).toContain("Features: 0/1");
      // m1 has 3 assertions, m2 has 1 assertion
      expect(stdout).toContain("Assertions:");
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone list errors for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["milestone", "list", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("milestone status", () => {
    it("milestone status <mid> --mission <id> shows detailed status", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["milestone", "status", "m1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Milestone: m1");
      expect(stdout).toContain("Milestone 1");
      expect(stdout).toContain("Features:");
      expect(stdout).toContain("Assertions:");
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone status --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["milestone", "status", "m1", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.milestone).toBeDefined();
      expect(result.milestone.id).toBe("m1");
      expect(result.progress).toBeDefined();
      expect(result.progress.featureCount).toBeDefined();
      expect(result.progress.assertionCount).toBeDefined();
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone status errors for non-existent milestone", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["milestone", "status", "nonexistent", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Milestone nonexistent not found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("milestone seal", () => {
    it("milestone seal succeeds when all assertions are passed", async () => {
      const missionId = await createMission(tmpDir);

      // Get assertions and pass them all
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;

      // Pass all assertions for m1
      for (const assertion of assertions) {
        await run(
          ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
          tmpDir,
        );
      }

      // Seal should succeed
      const { stdout, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Milestone sealed");
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone seal succeeds when all assertions are passed or waived", async () => {
      const missionId = await createMission(tmpDir);

      // Get assertions
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;

      // Pass first assertion, waive the rest
      for (let i = 0; i < assertions.length; i++) {
        if (i === 0) {
          await run(
            ["validate", "update", assertions[i]!.id, "--mission", missionId, "--result", "passed"],
            tmpDir,
          );
        } else {
          await run(
            [
              "validate", "update", assertions[i]!.id,
              "--mission", missionId,
              "--result", "waived",
              "--reason", "Not applicable",
            ],
            tmpDir,
          );
        }
      }

      // Seal should succeed and report waived assertions
      const { stdout, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Milestone sealed");
      expect(stdout).toContain("Waived");
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone seal fails and identifies blocking assertion IDs", async () => {
      const missionId = await createMission(tmpDir);

      // Get assertions
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      expect(assertions.length).toBeGreaterThan(0);

      // Pass only the first assertion, leave others pending
      await run(
        ["validate", "update", assertions[0]!.id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );

      // Seal should fail with blocking assertion IDs
      const { stdout, stderr, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Cannot seal milestone m1");
      expect(output).toContain("Blocking assertions");
      // Should list the non-terminal assertion IDs
      for (let i = 1; i < assertions.length; i++) {
        expect(output).toContain(assertions[i]!.id);
      }
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone seal --json outputs parseable JSON on success", async () => {
      const missionId = await createMission(tmpDir);

      // Get assertions and pass them all
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;

      // Pass all assertions
      for (const assertion of assertions) {
        await run(
          ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
          tmpDir,
        );
      }

      const { stdout, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.sealed).toBe(true);
      expect(result.milestone.id).toBe("m1");
      expect(result.blockingAssertionIds).toHaveLength(0);
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone seal --json outputs error JSON on failure", async () => {
      const missionId = await createMission(tmpDir);

      // Don't pass any assertions - seal should fail
      const { stdout, stderr, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      const result = JSON.parse(output);
      expect(result.error).toBeDefined();
      expect(result.hints).toBeDefined();
    }, SLOW_CLI_TIMEOUT_MS);

    it("milestone seal auto-transitions executing milestones", async () => {
      const missionId = await createMission(tmpDir);

      // Approve mission first
      await run(["mission", "approve", missionId], tmpDir);
      // Move to executing
      await run(["mission", "update", missionId, "--status", "executing"], tmpDir);

      // Get assertions and pass them all
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;

      for (const assertion of assertions) {
        await run(
          ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
          tmpDir,
        );
      }

      // Seal should auto-transition and succeed
      const { stdout, exitCode } = await run(
        ["milestone", "seal", "m1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Milestone sealed");
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from different positions for milestone list", async () => {
      const missionId = await createMission(tmpDir);

      // Root position
      const rootResult = await run(
        ["--json", "milestone", "list", "--mission", missionId],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position
      const groupResult = await run(
        ["milestone", "--json", "list", "--mission", missionId],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from different positions for milestone status", async () => {
      const missionId = await createMission(tmpDir);

      // Root position
      const rootResult = await run(
        ["--json", "milestone", "status", "m1", "--mission", missionId],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position
      const groupResult = await run(
        ["milestone", "--json", "status", "m1", "--mission", missionId],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from different positions for milestone seal", async () => {
      const missionId = await createMission(tmpDir);

      // Pass all assertions first
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      for (const assertion of assertions) {
        await run(
          ["validate", "update", assertion.id, "--mission", missionId, "--result", "passed"],
          tmpDir,
        );
      }

      // Root position
      const rootResult = await run(
        ["--json", "milestone", "seal", "m1", "--mission", missionId],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position
      const missionId2 = await createMission(tmpDir);
      const showResult2 = await run(
        ["validate", "show", "--mission", missionId2, "--milestone", "m1", "--json"],
        tmpDir,
      );
      const assertions2 = JSON.parse(showResult2.stdout).assertions;
      for (const assertion of assertions2) {
        await run(
          ["validate", "update", assertion.id, "--mission", missionId2, "--result", "passed"],
          tmpDir,
        );
      }
      
      const groupResult = await run(
        ["milestone", "--json", "seal", "m1", "--mission", missionId2],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);
  });
});
