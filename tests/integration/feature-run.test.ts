import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startA2aTestServer, type TestA2aServer } from "../helpers/a2a-test-server.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;
let a2aServer: TestA2aServer | undefined;

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

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode: await proc.exited,
  };
}

async function initGitRepo(cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
}

async function createMission(cwd: string): Promise<string> {
  const plan = {
    title: "Run Mission",
    description: "desc",
    milestones: [
      { id: "m1", title: "Milestone 1", description: "desc", order: 0 },
    ],
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "desc",
        workerType: "test-skill",
        verificationSteps: ["step-1"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "desc",
        workerType: "test-skill",
        verificationSteps: ["step-2"],
        dependsOn: ["f1"],
      },
    ],
  };
  const planPath = join(cwd, "plan.json");
  await writeFile(planPath, JSON.stringify(plan, null, 2));

  const { stdout } = await run(["mission", "create", "--file", planPath, "--json"], cwd);
  return JSON.parse(stdout).mission.id;
}

async function writeSkillAndConfig(cwd: string): Promise<void> {
  await mkdir(join(cwd, ".maestro", "skills", "test-skill"), { recursive: true });
  await writeFile(join(cwd, ".maestro", "skills", "test-skill", "SKILL.md"), "# test skill\n");

  const workerScript = join(cwd, "worker.ts");
  await writeFile(
    workerScript,
    [
      "const report = {",
      "  salientSummary: 'worker ok',",
      "  whatWasImplemented: 'implemented',",
      "  whatWasLeftUndone: '',",
      "  verification: { commandsRun: [], interactiveChecks: [] },",
      "  tests: { added: [] },",
      "  discoveredIssues: [],",
      "};",
      "console.log(JSON.stringify(report));",
    ].join("\n"),
  );

  await writeFile(
    join(cwd, ".maestro", "config.yaml"),
    [
      "execution:",
      "  defaultWorker: test-worker",
      "workers:",
      "  test-worker:",
      "    enabled: true",
      "    transport: cli",
      "    command: bun",
      `    args: ["${workerScript}"]`,
      "    outputMode: raw",
    ].join("\n"),
  );
}

async function writeA2aConfig(cwd: string, baseUrl: string): Promise<void> {
  await mkdir(join(cwd, ".maestro", "skills", "test-skill"), { recursive: true });
  await writeFile(join(cwd, ".maestro", "skills", "test-skill", "SKILL.md"), "# test skill\n");

  await writeFile(
    join(cwd, ".maestro", "config.yaml"),
    [
      "execution:",
      "  defaultWorker: test-worker",
      "  allowA2a: true",
      "workers:",
      "  test-worker:",
      "    enabled: true",
      "    transport: a2a",
      `    url: ${baseUrl}`,
    ].join("\n"),
  );
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-feature-run-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await a2aServer?.close();
  a2aServer = undefined;
  await rm(tmpDir, { recursive: true, force: true });
});

describe("feature run integration", () => {
  it("supports dry-run json output", async () => {
    const missionId = await createMission(tmpDir);
    await writeSkillAndConfig(tmpDir);

    const { stdout, exitCode } = await run(
      ["feature", "run", "--mission", missionId, "--dry-run", "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const result = JSON.parse(stdout);
    expect(result.dryRun).toBe(true);
    expect(result.outcomes[0].status).toBe("dry-run");
  });

  it("allows a real run immediately after dry-run", async () => {
    const missionId = await createMission(tmpDir);
    await writeSkillAndConfig(tmpDir);

    const dryRunResult = await run(
      ["feature", "run", "--mission", missionId, "--dry-run", "--json"],
      tmpDir,
    );
    expect(dryRunResult.exitCode).toBe(0);

    const realRunResult = await run(
      ["feature", "run", "--mission", missionId, "--worker", "test-worker"],
      tmpDir,
    );

    expect(realRunResult.exitCode).toBe(0);
    expect(realRunResult.stdout).toContain("Feature run finished");
  });

  it("runs features through the configured worker", async () => {
    const missionId = await createMission(tmpDir);
    await writeSkillAndConfig(tmpDir);

    const { stdout, exitCode } = await run(
      ["feature", "run", "--mission", missionId, "--worker", "test-worker"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Feature run finished");
    expect(stdout).toContain("test-worker");

    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const features = JSON.parse(listResult.stdout).features;
    expect(features.every((feature: { status: string }) => feature.status === "done")).toBe(true);
  });

  it("runs features through a live A2A worker", async () => {
    const missionId = await createMission(tmpDir);
    a2aServer = await startA2aTestServer("a2a integration ok");
    await writeA2aConfig(tmpDir, a2aServer.baseUrl);

    const { stdout, exitCode } = await run(
      ["feature", "run", "--mission", missionId, "--worker", "test-worker"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("Feature run finished");
    expect(stdout).toContain("a2a integration ok");

    const listResult = await run(
      ["feature", "list", "--mission", missionId, "--json"],
      tmpDir,
    );
    const features = JSON.parse(listResult.stdout).features;
    expect(features.every((feature: { status: string }) => feature.status === "done")).toBe(true);
  });

  it("rejects A2A workers until execution.allowA2a is enabled explicitly", async () => {
    const missionId = await createMission(tmpDir);
    a2aServer = await startA2aTestServer("a2a integration ok");
    await mkdir(join(tmpDir, ".maestro", "skills", "test-skill"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro", "skills", "test-skill", "SKILL.md"), "# test skill\n");
    await writeFile(
      join(tmpDir, ".maestro", "config.yaml"),
      [
        "execution:",
        "  defaultWorker: test-worker",
        "workers:",
        "  test-worker:",
        "    enabled: true",
        "    transport: a2a",
        `    url: ${a2aServer.baseUrl}`,
      ].join("\n"),
    );

    const { stdout, stderr, exitCode } = await run(
      ["feature", "run", "--mission", missionId, "--worker", "test-worker"],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    expect(`${stdout}\n${stderr}`).toContain("execution.allowA2a: true");
  });

  it("fails fast when config yaml is malformed", async () => {
    const missionId = await createMission(tmpDir);
    await mkdir(join(tmpDir, ".maestro"), { recursive: true });
    await writeFile(join(tmpDir, ".maestro", "config.yaml"), "execution: [broken");

    const { stdout, stderr, exitCode } = await run(
      ["feature", "run", "--mission", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    expect(`${stdout}\n${stderr}`).toContain("Cannot load Maestro config due to YAML errors");
  });
});
