/**
 * Integration tests for mission CLI commands
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
    description: "A test mission for CLI integration",
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
        milestoneId: "m2",
        title: "Feature 2",
        description: "Second feature",
        skillName: "test-skill",
        verificationSteps: ["step3"],
      },
    ],
  };
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("mission CLI commands", () => {
  it("mission create --file plan.json creates a mission", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const { stdout, exitCode } = await run(
      ["mission", "create", "--file", planPath],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission created:");
    expect(stdout).toContain("Test Mission");
    expect(stdout).toContain("Status: draft");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission create --file - reads from stdin", async () => {
    const plan = createSamplePlan();
    const planJson = JSON.stringify(plan);

    // Write plan JSON to stdin using Bun.spawn with string input
    const proc = Bun.spawn([...CLI, "mission", "create", "--file", "-"], {
      stdout: "pipe",
      stderr: "pipe",
      stdin: new Response(planJson).body,
      cwd: tmpDir,
    });

    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);
    const exitCode = await proc.exited;

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission created:");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission create --file plan.json --json outputs JSON", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const { stdout, exitCode } = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.mission).toBeDefined();
    expect(result.mission.id).toMatch(/^\d{4}-\d{2}-\d{2}-\d{3}$/);
    expect(result.mission.status).toBe("draft");
    expect(result.features).toHaveLength(2);
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission create validates cross-references", async () => {
    const plan = {
      title: "Bad Mission",
      milestones: [{ id: "m1", title: "M1", description: "", order: 0 }],
      features: [
        {
          id: "f1",
          milestoneId: "nonexistent", // dangling reference
          title: "Bad Feature",
          description: "",
          skillName: "test",
          verificationSteps: ["step"],
        },
      ],
    };
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const { stdout, stderr, exitCode } = await run(
      ["mission", "create", "--file", planPath],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("references non-existent milestone");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission list shows created missions", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    // Create two missions
    await run(["mission", "create", "--file", planPath], tmpDir);
    await run(["mission", "create", "--file", planPath], tmpDir);

    const { stdout, exitCode } = await run(["mission", "list"], tmpDir);

    expect(exitCode).toBe(0);
    expect(stdout).toContain("2 mission(s)");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission list --status filters by status", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    // Create a mission
    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    // Approve it
    await run(["mission", "approve", missionId], tmpDir);

    // List only draft (should be empty)
    const draftList = await run(["mission", "list", "--status", "draft"], tmpDir);
    expect(draftList.stdout).toContain("No missions found");

    // List approved
    const approvedList = await run(["mission", "list", "--status", "approved"], tmpDir);
    expect(approvedList.stdout).toContain("1 mission(s)");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission show <id> displays mission details", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    const { stdout, exitCode } = await run(
      ["mission", "show", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission:");
    expect(stdout).toContain(missionId);
    expect(stdout).toContain("Test Mission");
    expect(stdout).toContain("Milestones (2)");
    expect(stdout).toContain("Features (2)");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission show --json outputs parseable JSON", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    const { stdout, exitCode } = await run(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const report = JSON.parse(stdout);
    expect(report.mission).toBeDefined();
    expect(report.mission.id).toBe(missionId);
    expect(report.mission.title).toBe("Test Mission");
    expect(report.milestones).toHaveLength(2);
    expect(report.summary).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission approve transitions draft to approved", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    const { stdout, exitCode } = await run(
      ["mission", "approve", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission approved");
    expect(stdout).toContain("Approved at:");

    // Verify by showing
    const showResult = await run(["mission", "show", missionId, "--json"], tmpDir);
    const report = JSON.parse(showResult.stdout);
    expect(report.mission.status).toBe("approved");
    expect(report.mission.approvedAt).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission reject transitions draft to rejected", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    const { stdout, exitCode } = await run(
      ["mission", "reject", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission rejected");
    expect(stdout).toContain("Rejected at:");

    // Verify by showing
    const showResult = await run(["mission", "show", missionId, "--json"], tmpDir);
    const report = JSON.parse(showResult.stdout);
    expect(report.mission.status).toBe("rejected");
    expect(report.mission.rejectedAt).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission update --status updates status with legal transitions", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    // Approve first
    await run(["mission", "approve", missionId], tmpDir);

    // Then update to executing
    const { stdout, exitCode } = await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Mission updated");
    expect(stdout).toContain("executing");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission update --status rejects illegal transitions", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    // Try to go directly from draft to executing (illegal)
    const { stdout, stderr, exitCode } = await run(
      ["mission", "update", missionId, "--status", "executing"],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("Invalid mission transition");
    expect(output).toContain("approved"); // should suggest approved as valid next state
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission update --title updates the title", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    const { stdout, exitCode } = await run(
      ["mission", "update", missionId, "--title", "Updated Title"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Updated Title");

    // Verify
    const showResult = await run(["mission", "show", missionId, "--json"], tmpDir);
    const report = JSON.parse(showResult.stdout);
    expect(report.mission.title).toBe("Updated Title");
  }, SLOW_CLI_TIMEOUT_MS);

  it("JSON output works from different flag positions", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    // Root position: maestro --json mission create --file plan.json
    const rootResult = await run(
      ["--json", "mission", "create", "--file", planPath],
      tmpDir,
    );
    expect(rootResult.exitCode).toBe(0);
    expect(() => JSON.parse(rootResult.stdout)).not.toThrow();

    // Group position: maestro mission --json create --file plan.json
    const groupResult = await run(
      ["mission", "--json", "create", "--file", planPath],
      tmpDir,
    );
    expect(groupResult.exitCode).toBe(0);
    expect(() => JSON.parse(groupResult.stdout)).not.toThrow();
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission show errors with helpful hints for non-existent mission", async () => {
    const { stdout, stderr, exitCode } = await run(
      ["mission", "show", "2026-03-28-001"],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("Mission 2026-03-28-001 not found");
    expect(output).toContain("maestro mission list");
  }, SLOW_CLI_TIMEOUT_MS);

  it("mission approve errors with hints for invalid transitions", async () => {
    const plan = createSamplePlan();
    const planPath = join(tmpDir, "plan.json");
    await writeFile(planPath, JSON.stringify(plan, null, 2));

    const createResult = await run(
      ["mission", "create", "--file", planPath, "--json"],
      tmpDir,
    );
    const missionId = JSON.parse(createResult.stdout).mission.id;

    // Approve once
    await run(["mission", "approve", missionId], tmpDir);

    // Try to approve again (invalid from approved state)
    const { stdout, stderr, exitCode } = await run(
      ["mission", "approve", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    const output = stdout + stderr;
    expect(output).toContain("Invalid mission transition");
    expect(output).toContain("Valid transitions from approved");
  }, SLOW_CLI_TIMEOUT_MS);
});
