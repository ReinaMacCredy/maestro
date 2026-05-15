/**
 * Integration E2E tests for Phase 7 reconciliation
 * Validates new state transitions, field roundtrips, and CLI workflows
 * added during the plan/implementation reconciliation.
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

function createBasicPlan(): object {
  return {
    title: "Reconciliation Test Mission",
    description: "E2E test for reconciliation scenarios",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        agentType: "test-skill",
        verificationSteps: ["step1"],
        fulfills: ["assertion-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        agentType: "test-skill",
        verificationSteps: ["step2"],
        fulfills: ["assertion-2"],
      },
    ],
  };
}

function createRichFieldsPlan(): object {
  return {
    title: "Rich Fields Mission",
    description: "Tests new fields from reconciliation",
    proposal: "# Full proposal\n\nDetailed implementation plan with milestones.",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature With Rich Fields",
        description: "Tests fulfills, preconditions, expectedBehavior",
        agentType: "test-skill",
        verificationSteps: ["Verify DB connection", "Check HTTP 200"],
        fulfills: ["assertion-db", "assertion-http"],
        preconditions: "PostgreSQL running on port 5432",
        expectedBehavior: "Returns 200 OK with JSON body containing user data",
      },
    ],
  };
}

async function createMission(cwd: string, plan?: object): Promise<string> {
  const planData = plan ?? createBasicPlan();
  const planPath = join(cwd, "plan.json");
  await writeFile(planPath, JSON.stringify(planData, null, 2));

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
    `# ${skillName}\n\nThis is a test skill for ${skillName}.\n`,
  );
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-reconciliation-"));
  await initGitRepo(tmpDir);
  await createSkill(tmpDir, "test-skill");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("reconciliation E2E: pause/resume flow", () => {
  it("pauses and resumes an executing mission", async () => {
    const missionId = await createMission(tmpDir);

    // Approve and start executing
    await run(["mission", "approve", missionId], tmpDir);
    await run(["mission", "update", missionId, "--status", "executing"], tmpDir);

    // Pause the mission
    const pauseResult = await run(
      ["mission", "update", missionId, "--status", "paused", "--json"],
      tmpDir,
    );
    expect(pauseResult.exitCode).toBe(0);
    expect(JSON.parse(pauseResult.stdout).status).toBe("paused");

    // Verify via show
    const showPaused = await run(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );
    expect(showPaused.exitCode).toBe(0);
    expect(JSON.parse(showPaused.stdout).mission.status).toBe("paused");

    // Resume to executing
    const resumeResult = await run(
      ["mission", "update", missionId, "--status", "executing", "--json"],
      tmpDir,
    );
    expect(resumeResult.exitCode).toBe(0);
    expect(JSON.parse(resumeResult.stdout).status).toBe("executing");

    // Continue through normal lifecycle to completion
    // Move features through their lifecycle
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "done"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "in-progress"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    await run(
      ["feature", "update", "f2", "--mission", missionId, "--status", "done"],
      tmpDir,
    );

    // Pass all assertions
    const assertions = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    for (const a of JSON.parse(assertions.stdout).assertions) {
      await run(
        ["validate", "update", a.id, "--mission", missionId, "--result", "passed"],
        tmpDir,
      );
    }

    // Transition to validating then completed
    await run(["mission", "update", missionId, "--status", "validating"], tmpDir);
    const completeResult = await run(
      ["mission", "update", missionId, "--status", "completed", "--json"],
      tmpDir,
    );
    expect(completeResult.exitCode).toBe(0);
    expect(JSON.parse(completeResult.stdout).status).toBe("completed");
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects paused -> completed (must resume first)", async () => {
    const missionId = await createMission(tmpDir);

    await run(["mission", "approve", missionId], tmpDir);
    await run(["mission", "update", missionId, "--status", "executing"], tmpDir);
    await run(["mission", "update", missionId, "--status", "paused"], tmpDir);

    // Try to go directly from paused to completed
    const result = await run(
      ["mission", "update", missionId, "--status", "completed"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    const output = result.stdout + result.stderr;
    expect(output).toContain("Invalid mission transition");
    expect(output).toContain("executing"); // hint should suggest executing
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("reconciliation E2E: assigned feature state", () => {
  it("transitions feature through assigned state", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    // Transition to assigned
    const assignResult = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "assigned", "--json"],
      tmpDir,
    );
    expect(assignResult.exitCode).toBe(0);
    expect(JSON.parse(assignResult.stdout).feature.status).toBe("assigned");

    // Verify via list
    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const features = JSON.parse(listResult.stdout).features;
    const f1 = features.find((f: { id: string }) => f.id === "f1");
    expect(f1.status).toBe("assigned");

    // Continue to in-progress
    const progressResult = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "in-progress", "--json"],
      tmpDir,
    );
    expect(progressResult.exitCode).toBe(0);
    expect(JSON.parse(progressResult.stdout).feature.status).toBe("in-progress");

    // Complete normally
    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    const doneResult = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "done", "--json"],
      tmpDir,
    );
    expect(doneResult.exitCode).toBe(0);
    expect(JSON.parse(doneResult.stdout).feature.status).toBe("done");
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects assigned -> review (must go through in-progress)", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "assigned"],
      tmpDir,
    );

    const result = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "review"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    const output = result.stdout + result.stderr;
    expect(output).toContain("Invalid feature transition");
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("reconciliation E2E: blocked -> waived assertion", () => {
  it("transitions assertion from blocked directly to waived", async () => {
    const missionId = await createMission(tmpDir);

    // Get assertion ID
    const showResult = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    const assertions = JSON.parse(showResult.stdout).assertions;
    const assertionId = assertions[0]!.id;

    // Block the assertion
    const blockResult = await run(
      [
        "validate", "update", assertionId,
        "--mission", missionId,
        "--result", "blocked",
        "--evidence", "External dependency unavailable",
      ],
      tmpDir,
    );
    expect(blockResult.exitCode).toBe(0);

    // Waive directly from blocked
    const waiveResult = await run(
      [
        "validate", "update", assertionId,
        "--mission", missionId,
        "--result", "waived",
        "--reason", "Not automatable - manual verification done",
      ],
      tmpDir,
    );
    expect(waiveResult.exitCode).toBe(0);

    // Verify
    const verifyResult = await run(
      ["validate", "show", "--mission", missionId, "--json"],
      tmpDir,
    );
    const updatedAssertions = JSON.parse(verifyResult.stdout).assertions;
    const waived = updatedAssertions.find((a: { id: string }) => a.id === assertionId);
    expect(waived.result).toBe("waived");
    expect(waived.waivedReason).toBe("Not automatable - manual verification done");
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("reconciliation E2E: rich AgentReport via CLI", () => {
  it("roundtrips rich agent report format", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);

    const richReport = {
      salientSummary: "Implemented database connection pooling",
      whatWasImplemented: "Connection pool with configurable max connections",
      whatWasLeftUndone: "Connection retry logic deferred to next milestone",
      verification: {
        commandsRun: [
          { command: "bun test", exitCode: 0, observation: "All 42 tests pass" },
        ],
        interactiveChecks: [
          { action: "Opened browser to /health", observed: "200 OK with pool stats" },
        ],
      },
      tests: {
        added: [
          {
            file: "tests/pool.test.ts",
            cases: [
              { name: "creates pool with default config", verifies: "pool initialization" },
              { name: "respects max connections", verifies: "connection limit" },
            ],
          },
        ],
      },
      discoveredIssues: [
        {
          severity: "medium",
          description: "Connection timeout default too aggressive",
          suggestedFix: "Increase from 5s to 30s",
        },
      ],
    };

    const updateResult = await run(
      [
        "feature", "update", "f1",
        "--mission", missionId,
        "--status", "in-progress",
        "--report", JSON.stringify(richReport),
        "--json",
      ],
      tmpDir,
    );
    expect(updateResult.exitCode).toBe(0);

    // Read back via feature list JSON
    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(listResult.exitCode).toBe(0);
    const features = JSON.parse(listResult.stdout).features;
    const f1 = features.find((f: { id: string }) => f.id === "f1");
    expect(f1.report).toBeDefined();
    expect(f1.report.salientSummary).toBe("Implemented database connection pooling");
    expect(f1.report.whatWasImplemented).toBe("Connection pool with configurable max connections");
    expect(f1.report.whatWasLeftUndone).toBe("Connection retry logic deferred to next milestone");
    expect(f1.report.verification.commandsRun).toHaveLength(1);
    expect(f1.report.verification.commandsRun[0].command).toBe("bun test");
    expect(f1.report.verification.interactiveChecks).toHaveLength(1);
    expect(f1.report.tests.added).toHaveLength(1);
    expect(f1.report.tests.added[0].cases).toHaveLength(2);
    expect(f1.report.discoveredIssues).toHaveLength(1);
    expect(f1.report.discoveredIssues[0].severity).toBe("medium");
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("reconciliation E2E: feature with new plan fields", () => {
  it("creates mission with fulfills, preconditions, expectedBehavior and reads them back", async () => {
    const missionId = await createMission(tmpDir, createRichFieldsPlan());

    // Verify fields in feature list output
    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(listResult.exitCode).toBe(0);
    const features = JSON.parse(listResult.stdout).features;
    expect(features).toHaveLength(1);

    const f1 = features[0];
    expect(f1.fulfills).toEqual(["assertion-db", "assertion-http"]);
    expect(f1.preconditions).toBe("PostgreSQL running on port 5432");
    expect(f1.expectedBehavior).toBe("Returns 200 OK with JSON body containing user data");
  }, SLOW_CLI_TIMEOUT_MS);

  it("preserves proposal field on mission through lifecycle", async () => {
    const missionId = await createMission(tmpDir, createRichFieldsPlan());

    // Show mission and verify proposal
    const showResult = await run(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );
    expect(showResult.exitCode).toBe(0);
    const mission = JSON.parse(showResult.stdout).mission;
    expect(mission.proposal).toBe("# Full proposal\n\nDetailed implementation plan with milestones.");

    // Approve and verify proposal persists
    await run(["mission", "approve", missionId], tmpDir);
    const showApproved = await run(
      ["mission", "show", missionId, "--json"],
      tmpDir,
    );
    expect(JSON.parse(showApproved.stdout).mission.proposal).toBe(
      "# Full proposal\n\nDetailed implementation plan with milestones.",
    );
  }, SLOW_CLI_TIMEOUT_MS);
});

describe("reconciliation E2E: milestone validating -> executing retry", () => {
  it("milestone can retry from validating to executing via milestone seal failure and re-attempt", async () => {
    const missionId = await createMission(tmpDir);
    await run(["mission", "approve", missionId], tmpDir);
    await run(["mission", "update", missionId, "--status", "executing"], tmpDir);

    // Complete all features in m1
    for (const fid of ["f1", "f2"]) {
      await run(
        ["feature", "update", fid, "--mission", missionId, "--status", "in-progress"],
        tmpDir,
      );
      await run(
        ["feature", "update", fid, "--mission", missionId, "--status", "review"],
        tmpDir,
      );
      await run(
        ["feature", "update", fid, "--mission", missionId, "--status", "done"],
        tmpDir,
      );
    }

    // Get assertions for m1
    const assertionsResult = await run(
      ["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"],
      tmpDir,
    );
    const assertions = JSON.parse(assertionsResult.stdout).assertions;

    // Pass first assertion, fail second
    await run(
      ["validate", "update", assertions[0]!.id, "--mission", missionId, "--result", "passed"],
      tmpDir,
    );
    await run(
      [
        "validate", "update", assertions[1]!.id,
        "--mission", missionId,
        "--result", "failed",
        "--evidence", "Test flake - intermittent timeout",
      ],
      tmpDir,
    );

    // Seal attempt should fail because of the failed assertion
    const sealFail = await run(
      ["milestone", "seal", "m1", "--mission", missionId],
      tmpDir,
    );
    expect(sealFail.exitCode).toBe(1);
    const failOutput = sealFail.stdout + sealFail.stderr;
    expect(failOutput).toContain("Cannot seal milestone");

    // Retry the failed assertion: failed -> pending -> passed
    await run(
      ["validate", "update", assertions[1]!.id, "--mission", missionId, "--result", "pending"],
      tmpDir,
    );
    await run(
      [
        "validate", "update", assertions[1]!.id,
        "--mission", missionId,
        "--result", "passed",
        "--evidence", "Retry succeeded after fixing timeout",
      ],
      tmpDir,
    );

    // Now seal should succeed
    const sealOk = await run(
      ["milestone", "seal", "m1", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(sealOk.exitCode).toBe(0);
    expect(JSON.parse(sealOk.stdout).sealed).toBe(true);
  }, SLOW_CLI_TIMEOUT_MS);

  it("compact progress notation in mission show text output", async () => {
    // Create mission with 2 milestones
    const plan = {
      title: "Compact Test",
      milestones: [
        { id: "m1", title: "M1", description: "First", order: 0 },
        { id: "m2", title: "M2", description: "Second", order: 1 },
      ],
      features: [
        { id: "f1", milestoneId: "m1", title: "F1", description: "D", agentType: "w", verificationSteps: ["S"], fulfills: ["A1"] },
        { id: "f2", milestoneId: "m2", title: "F2", description: "D", agentType: "w", verificationSteps: ["S"], fulfills: ["A2"] },
      ],
    };
    const planPath = join(tmpDir, "compact-plan.json");
    await writeFile(planPath, JSON.stringify(plan));

    const createRes = await run(["mission", "create", "--file", planPath, "--json"], tmpDir);
    expect(createRes.exitCode).toBe(0);
    const missionId = JSON.parse(createRes.stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["mission", "update", missionId, "--status", "executing", "--json"], tmpDir);

    // Complete m1
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "review"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "done"], tmpDir);

    // Pass m1 assertion and seal
    const showRes = await run(["validate", "show", "--mission", missionId, "--milestone", "m1", "--json"], tmpDir);
    const m1Assertions = JSON.parse(showRes.stdout).assertions;
    for (const a of m1Assertions) {
      await run(["validate", "update", a.id, "--mission", missionId, "--result", "passed"], tmpDir);
    }
    await run(["milestone", "seal", "m1", "--mission", missionId, "--json"], tmpDir);

    // Start m2
    await run(["feature", "update", "f2", "--mission", missionId, "--status", "in-progress"], tmpDir);

    // Text output should have compact progress
    const textRes = await run(["mission", "show", missionId], tmpDir);
    expect(textRes.exitCode).toBe(0);
    const output = textRes.stdout;
    expect(output).toContain("Progress:");
    expect(output).toContain("[m1: sealed]");
    expect(output).toMatch(/\[m2: executing \d+\/\d+\]/);
  }, SLOW_CLI_TIMEOUT_MS);

  it("--retry-reason persists to retry log", async () => {
    const plan = createBasicPlan();
    const planPath = join(tmpDir, "retry-plan.json");
    await writeFile(planPath, JSON.stringify(plan));

    const createRes = await run(["mission", "create", "--file", planPath, "--json"], tmpDir);
    expect(createRes.exitCode).toBe(0);
    const missionId = JSON.parse(createRes.stdout).mission.id;

    await run(["mission", "approve", missionId, "--json"], tmpDir);
    await run(["mission", "update", missionId, "--status", "executing", "--json"], tmpDir);

    // Progress feature to review then retry with reason
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "in-progress"], tmpDir);
    await run(["feature", "update", "f1", "--mission", missionId, "--status", "review"], tmpDir);
    const retryRes = await run(
      ["feature", "update", "f1", "--mission", missionId, "--status", "pending", "--retry-reason", "Tests failed on CI"],
      tmpDir,
    );
    expect(retryRes.exitCode).toBe(0);

    // Verify retry log file exists
    const { readFile } = await import("node:fs/promises");
    const retryLogPath = join(tmpDir, ".maestro", "missions", missionId, "agents", "f1", "retry-log.json");
    const logContent = JSON.parse(await readFile(retryLogPath, "utf-8"));
    expect(Array.isArray(logContent)).toBe(true);
    expect(logContent).toHaveLength(1);
    expect(logContent[0].reason).toBe("Tests failed on CI");
    expect(logContent[0].previousStatus).toBe("review");
    expect(logContent[0].timestamp).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);
});
