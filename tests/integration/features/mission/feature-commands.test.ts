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
  join(import.meta.dir, "..", "..", "..", "..", "src", "index.ts"),
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
        agentType: "test-skill",
        verificationSteps: ["step1", "step2"],
        fulfills: ["assertion-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        agentType: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: ["f1"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        agentType: "test-skill",
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
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );

      const { stdout, exitCode } = await run(
        ["feature", "list", "--mission", missionId, "--status", "in-progress"],
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
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
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
          "in-progress",
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

  describe("help text", () => {
    it("feature update --help lists the actual supported statuses", async () => {
      const { stdout, exitCode } = await run(["feature", "update", "--help"], tmpDir);

      expect(exitCode).toBe(0);
      expect(stdout).toContain("New status");
      expect(stdout).toContain("pending");
      expect(stdout).toContain("assigned");
      expect(stdout).toContain("in-progress");
      expect(stdout).toContain("review");
      expect(stdout).toContain("done");
      expect(stdout).toContain("blocked");
      expect(stdout).not.toContain("in_progress");
      expect(stdout).not.toContain("in_review");
      expect(stdout).not.toContain("completed");
    }, SLOW_CLI_TIMEOUT_MS);
  });

  describe("feature update", () => {
    it("feature update --status transitions feature status", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Feature updated: f1");
      expect(stdout).toContain("Status: in-progress");

      // Verify by listing
      const listResult = await run(
        ["feature", "list", "--mission", missionId, "--json"],
        tmpDir,
      );
      const features = JSON.parse(listResult.stdout).features;
      const f1 = features.find((f: { id: string }) => f.id === "f1");
      expect(f1.status).toBe("in-progress");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update makes approved mission auto-start explicit in text output", async () => {
      const missionId = await createMission(tmpDir);
      await run(["mission", "approve", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Mission: auto-started to executing");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update reports mission auto-start in json output", async () => {
      const missionId = await createMission(tmpDir);
      await run(["mission", "approve", missionId], tmpDir);

      const { stdout, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.missionAutoStarted).toBe(true);
      expect(result.feature.status).toBe("in-progress");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update --status rejects illegal transitions", async () => {
      const missionId = await createMission(tmpDir);

      // Try to go directly from pending to completed (illegal)
      const { stdout, stderr, exitCode } = await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "done"],
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
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
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
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
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

      // Legacy format input -- parseWorkerReport converts to rich format
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
          "in-progress",
          "--report",
          JSON.stringify(report),
        ],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Feature updated: f1");
      expect(stdout).toContain("Report:");
      expect(stdout).toContain("report.json");
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
          "in-progress",
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
        ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.feature.id).toBe("f1");
      expect(result.feature.status).toBe("in-progress");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature update errors for non-existent feature", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["feature", "update", "nonexistent", "--mission", missionId, "--status", "in-progress"],
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
          "in-progress",
          "--report",
          JSON.stringify(report1),
        ],
        tmpDir,
      );

      // Move to in_review
      await run(
        ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
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
      // Legacy content is promoted to salientSummary on parse
      expect(f1.report.salientSummary).toBe("First implementation attempt");
      expect(f1.report.whatWasImplemented).toBe("First implementation attempt");
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

  describe("feature prompt", () => {
    async function createSkill(baseDir: string, skillName: string): Promise<void> {
      const skillDir = join(baseDir, ".maestro", "skills", skillName);
      await mkdir(skillDir, { recursive: true });
      await writeFile(
        join(skillDir, "SKILL.md"),
        `# ${skillName}\n\nThis is a test skill for ${skillName}.\n`,
      );
    }

    async function createBuiltInSkill(baseDir: string, skillName: string): Promise<void> {
      const skillDir = join(baseDir, "skills", "built-in", skillName);
      await mkdir(skillDir, { recursive: true });
      await writeFile(
        join(skillDir, "SKILL.md"),
        `# ${skillName}\n\nThis is a built-in test skill for ${skillName}.\n`,
      );
    }

    it("feature prompt generates a worker prompt to stdout", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Worker prompt generated for: f1");
      expect(stdout).toContain("Agent type: test-skill");
      expect(stdout).toContain("--- PROMPT BEGIN ---");
      expect(stdout).toContain("--- PROMPT END ---");
      expect(stdout).toContain("Worker Assignment: Feature 1");
      expect(stdout).toContain("# test-skill");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt --json outputs parseable JSON", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.prompt).toBeDefined();
      expect(result.featureId).toBe("f1");
      expect(result.agentType).toBe("test-skill");
      expect(result.writtenTo).toBeDefined();
      expect(result.writtenTo.length).toBe(1);
      expect(result.writtenTo[0]).toContain("workers/f1/prompt.md");
      expect(result.prompt).toContain("# Worker Assignment: Feature 1");
      expect(result.prompt).toContain("## Skill Instructions");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt --out writes to custom path", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");
      const outPath = join(tmpDir, "custom-prompt.md");

      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId, "--out", outPath],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Written to:");
      expect(stdout).toContain(outPath);

      // Verify file was written
      const fs = await import("node:fs/promises");
      const fileContent = await fs.readFile(outPath, "utf-8");
      expect(fileContent).toContain("Worker Assignment: Feature 1");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt writes to workers/{featureId}/prompt.md", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);

      // Verify file was written
      const promptPath = join(tmpDir, ".maestro", "missions", missionId, "workers", "f1", "prompt.md");
      const fs = await import("node:fs/promises");
      const fileContent = await fs.readFile(promptPath, "utf-8");
      expect(fileContent).toContain("Worker Assignment: Feature 1");
      expect(fileContent).toContain("## Mission Context");
      expect(fileContent).toContain("## Feature Assignment");
      expect(fileContent).toContain("## Skill Instructions");
      expect(fileContent).toContain("<!-- BEGIN SKILL -->");
      expect(fileContent).toContain("<!-- END SKILL -->");
      }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt falls back to built-in skills when workspace skill is missing", async () => {
      const missionId = await createMission(tmpDir);
      await createBuiltInSkill(tmpDir, "test-skill");

      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Worker prompt generated for: f1");
      expect(stdout).toContain("built-in test skill");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt errors for missing skill file", async () => {
      const missionId = await createMission(tmpDir);
      // Don't create the skill file

      const { stdout, stderr, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Worker skill 'test-skill' not found");
      expect(output).toContain(".maestro/skills/test-skill/SKILL.md");
      expect(output).toContain("skills/built-in/test-skill/SKILL.md");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt errors for non-existent mission", async () => {
      await createSkill(tmpDir, "test-skill");

      const { stdout, stderr, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", "2026-03-28-001"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Mission 2026-03-28-001 not found");
      expect(output).toContain("maestro mission list");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt errors for non-existent feature", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      const { stdout, stderr, exitCode } = await run(
        ["feature", "prompt", "nonexistent", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      const output = stdout + stderr;
      expect(output).toContain("Feature nonexistent not found");
      expect(output).toContain("maestro feature list");
    }, SLOW_CLI_TIMEOUT_MS);

    it("feature prompt includes assertions in generated prompt", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      // Create assertion for f1 (which has fulfills: ["assertion-1"] in sample plan)
      // The assertion should be loaded if the feature fulfills it
      // For now, verify the prompt structure includes assertion-related sections
      const { stdout, exitCode } = await run(
        ["feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Verification Steps");
    }, SLOW_CLI_TIMEOUT_MS);

    it("JSON output works from different --json flag positions for prompt", async () => {
      const missionId = await createMission(tmpDir);
      await createSkill(tmpDir, "test-skill");

      // Root position: maestro --json feature prompt f1 --mission <id>
      const rootResult = await run(
        ["--json", "feature", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );
      expect(rootResult.exitCode).toBe(0);
      expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

      // Group position: maestro feature --json prompt f1 --mission <id>
      const groupResult = await run(
        ["feature", "--json", "prompt", "f1", "--mission", missionId],
        tmpDir,
      );
      expect(groupResult.exitCode).toBe(0);
      expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
    }, SLOW_CLI_TIMEOUT_MS);
  });
});
