import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;
let serverProc: Bun.Subprocess | undefined;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-a2a-command-"));
  const proc = Bun.spawn(["git", "init", "-b", "main"], {
    cwd: tmpDir,
    stdout: "pipe",
    stderr: "pipe",
  });
  await proc.exited;
});

afterEach(async () => {
  serverProc?.kill();
  serverProc = undefined;
  await rm(tmpDir, { recursive: true, force: true });
});

describe("a2a CLI command", () => {
  it("starts a demo server that feature run can use end-to-end", async () => {
    serverProc = Bun.spawn(
      [...CLI, "a2a", "serve-demo", "--port", "0", "--delay-ms", "25", "--step", "Planning", "--step", "Applying patch", "--step", "Done", "--json"],
      {
        cwd: tmpDir,
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    const startup = await readStartupJson(serverProc.stdout);
    expect(startup.baseUrl).toContain("http://127.0.0.1:");
    const cardResponse = await fetch(startup.agentCardUrl);
    expect(cardResponse.ok).toBe(true);

    const missionId = await createMission(tmpDir);
    await writeA2aConfig(tmpDir, startup.baseUrl);

    const runResult = await run(["feature", "run", "--mission", missionId, "--worker", "demo-a2a", "--json"], tmpDir);
    expect(runResult.exitCode).toBe(0);
    const payload = JSON.parse(runResult.stdout) as {
      success: boolean;
      outcomes: Array<{ status: string; worker: string }>;
    };
    expect(payload.success).toBe(true);
    expect(payload.outcomes.every((outcome) => outcome.status === "done")).toBe(true);
    expect(payload.outcomes.every((outcome) => outcome.worker === "demo-a2a")).toBe(true);
  });
});

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

async function createMission(cwd: string): Promise<string> {
  const plan = {
    title: "A2A Demo Mission",
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
    ],
  };
  const planPath = join(cwd, "plan.json");
  await writeFile(planPath, JSON.stringify(plan, null, 2));

  const { stdout, exitCode } = await run(["mission", "create", "--file", planPath, "--json"], cwd);
  expect(exitCode).toBe(0);
  return JSON.parse(stdout).mission.id;
}

async function writeA2aConfig(cwd: string, baseUrl: string): Promise<void> {
  await mkdir(join(cwd, ".maestro", "skills", "test-skill"), { recursive: true });
  await writeFile(join(cwd, ".maestro", "skills", "test-skill", "SKILL.md"), "# test skill\n");
  await writeFile(
    join(cwd, ".maestro", "config.yaml"),
    [
      "execution:",
      "  defaultWorker: demo-a2a",
      "workers:",
      "  demo-a2a:",
      "    enabled: true",
      "    transport: a2a",
      `    url: ${baseUrl}`,
    ].join("\n"),
  );
}

async function readStartupJson(
  stdout: ReadableStream<Uint8Array> | null,
): Promise<{ baseUrl: string; agentCardUrl: string; jsonRpcUrl: string }> {
  if (!stdout) {
    throw new Error("A2A demo server stdout is not readable");
  }

  const reader = stdout.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  const deadline = Date.now() + 10_000;

  while (Date.now() < deadline) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += value;
    const newlineIndex = buffer.indexOf("\n");
    if (newlineIndex >= 0) {
      const line = buffer.slice(0, newlineIndex).trim();
      if (line.length > 0) {
        reader.releaseLock();
        return JSON.parse(line) as { baseUrl: string; agentCardUrl: string; jsonRpcUrl: string };
      }
    }
  }

  reader.releaseLock();
  throw new Error(`Timed out waiting for A2A demo server startup output: ${buffer}`);
}
