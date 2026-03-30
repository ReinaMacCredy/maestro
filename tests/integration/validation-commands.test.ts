/**
 * Integration tests for validation CLI commands
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
    description: "A test mission for validation CLI integration",
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-validation-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("validation CLI commands", () => {
  describe("validate show", () => {
    it("validate show --mission <id> returns assertions for the mission", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["validate", "show", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("assertion(s)");
      expect(stdout).toContain("assertion-f1-1");
      expect(stdout).toContain("assertion-f1-2");
      expect(stdout).toContain("assertion-f2-1");
      expect(stdout).toContain("[f1]");
      expect(stdout).toContain("[f2]");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate show --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.assertions).toBeDefined();
      expect(Array.isArray(result.assertions)).toBe(true);
      expect(result.total).toBe(3);
      expect(result.filtered).toBe(3);
      expect(result.assertionCount).toBe(3);
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate show --milestone filters assertions by milestone", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "m1"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("milestone m1");
      // f1 and f2 are in m1, each with assertions
      expect(stdout).toContain("assertion-f1-1");
      expect(stdout).toContain("assertion-f2-1");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate show errors for non-existent mission", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["validate", "show", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
      expect(output).toContain("maestro mission list");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate show returns empty when no assertions match milestone filter", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["validate", "show", "--mission", missionId, "--milestone", "nonexistent"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("No assertions found");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("validate update", () => {
    it("validate update --result passed transitions assertion to passed", async () => {
      const missionId = await createMission(tmpDir);

      // Get the assertions list
      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const { stdout, exitCode } = await run(
        ["validate", "update", assertionId, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Assertion updated");
      expect(stdout).toContain("Result: passed");
      expect(stdout).toContain(assertionId);
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const { stdout, exitCode } = await run(
        ["validate", "update", assertionId, "--mission", missionId, "--result", "passed", "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.assertion).toBeDefined();
      expect(result.assertion.result).toBe("passed");
      expect(result.assertion.id).toBe(assertionId);
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update persists evidence with status", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const evidence = "Test execution passed with all checks";

      const { stdout, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "passed",
          "--evidence", evidence,
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Evidence:");
      expect(stdout).toContain(evidence);

      // Verify evidence persists in show output
      const showResult2 = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const updatedAssertions = JSON.parse(showResult2.stdout).assertions;
      const updated = updatedAssertions.find((a: { id: string }) => a.id === assertionId);
      expect(updated.evidence).toBe(evidence);
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update with --reason for waived status", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const reason = "Not applicable to this implementation";

      const { stdout, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "waived",
          "--reason", reason,
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Result: waived");
      expect(stdout).toContain("Waived Reason:");
      expect(stdout).toContain(reason);
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update rejects waive without reason", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const { stdout, stderr, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "waived",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("waivedReason is required when waiving an assertion");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update allows retry from failed to pending", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      // First fail the assertion
      await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "failed",
          "--evidence", "Initial failure",
        ],
        tmpDir,
      );

      // Then retry
      const { stdout, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "pending",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Result: pending");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update allows retry from blocked to pending", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      // First block the assertion
      await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "blocked",
          "--evidence", "Blocked by external dependency",
        ],
        tmpDir,
      );

      // Then retry
      const { stdout, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "pending",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Result: pending");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update rejects illegal transitions with helpful hints", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      // First pass the assertion
      await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "passed",
        ],
        tmpDir,
      );

      // Try to go back to pending
      const { stdout, stderr, exitCode } = await run(
        [
          "validate", "update", assertionId,
          "--mission", missionId,
          "--result", "pending",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Invalid assertion transition");
      expect(output).toContain("passed is a terminal state");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update errors for non-existent assertion", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        [
          "validate", "update", "nonexistent-assertion",
          "--mission", missionId,
          "--result", "passed",
        ],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Assertion nonexistent-assertion not found");
      expect(output).toContain("maestro validate show");
    }, SLOW_CLI_TIMEOUT_MS);

    it("validate update requires --result", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      const { stdout, stderr, exitCode } = await run(
        ["validate", "update", assertionId, "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("--result is required");
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from different positions for validate show", async () => {
      const missionId = await createMission(tmpDir);

      // Root position
      const rootResult = await run(
        ["--json", "validate", "show", "--mission", missionId],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position
      const groupResult = await run(
        ["validate", "--json", "show", "--mission", missionId],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON flag works from different positions for validate update", async () => {
      const missionId = await createMission(tmpDir);

      const showResult = await run(
        ["validate", "show", "--mission", missionId, "--json"],
        tmpDir,
      );
      const assertions = JSON.parse(showResult.stdout).assertions;
      const assertionId = assertions[0]!.id;

      // Root position
      const rootResult = await run(
        ["--json", "validate", "update", assertionId, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position
      const groupResult = await run(
        ["validate", "--json", "update", assertionId, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);
  });
});
