import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsExecutionStoreAdapter } from "../../src/adapters/execution-store.adapter.js";
import { FsRuntimeEventStoreAdapter } from "../../src/adapters/runtime-event-store.adapter.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

interface CliRunResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

interface SampledCliRunResult extends CliRunResult {
  readonly rssSamplesKb: readonly number[];
}

let tmpDir: string;
let a2aServer: Bun.Server | undefined;

async function run(
  args: readonly string[],
  cwd: string,
): Promise<CliRunResult> {
  const proc = Bun.spawn([...CLI, ...args], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode,
  };
}

async function runWithRssSampling(
  args: readonly string[],
  cwd: string,
): Promise<SampledCliRunResult> {
  const proc = Bun.spawn([...CLI, ...args], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  const rssSamplesKb: number[] = [];
  let completed = false;
  const sampler = (async () => {
    while (!completed) {
      const rssKb = await sampleRssKb(proc.pid);
      if (rssKb !== undefined) {
        rssSamplesKb.push(rssKb);
      }
      await Bun.sleep(50);
    }
  })();

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  completed = true;
  await sampler;

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
    exitCode,
    rssSamplesKb,
  };
}

async function sampleRssKb(pid: number): Promise<number | undefined> {
  const proc = Bun.spawn(["ps", "-o", "rss=", "-p", String(pid)], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    return undefined;
  }

  const value = stdout.trim();
  if (value.length === 0) {
    return undefined;
  }

  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function peakGrowthMb(samplesKb: readonly number[]): number {
  if (samplesKb.length < 2) {
    return 0;
  }

  const minKb = Math.min(...samplesKb);
  const maxKb = Math.max(...samplesKb);
  return (maxKb - minKb) / 1024;
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
    title: "Memory Mission",
    description: "stress worker output capture",
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

  const result = await run(["mission", "create", "--file", planPath, "--json"], cwd);
  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout).mission.id as string;
}

async function writeSkill(cwd: string): Promise<void> {
  await mkdir(join(cwd, ".maestro", "skills", "test-skill"), { recursive: true });
  await writeFile(join(cwd, ".maestro", "skills", "test-skill", "SKILL.md"), "# test skill\n");
}

async function writeCliWorkerConfig(cwd: string): Promise<void> {
  await writeSkill(cwd);
  const workerScript = join(cwd, "noisy-worker.ts");
  await writeFile(
    workerScript,
    [
      "console.log('first visible line');",
      "const payload = 'x'.repeat(768);",
      "for (let index = 0; index < 12000; index += 1) {",
      "  console.log(`noise-${index}-${payload}`);",
      "}",
      "console.log('final visible tail');",
    ].join("\n"),
  );

  await writeFile(
    join(cwd, ".maestro", "config.yaml"),
    [
      "execution:",
      "  defaultWorker: noisy-worker",
      "workers:",
      "  noisy-worker:",
      "    enabled: true",
      "    transport: cli",
      "    command: bun",
      `    args: ["${workerScript}"]`,
      "    outputMode: raw",
    ].join("\n"),
  );
}

async function startNoisyA2aServer(): Promise<{ readonly baseUrl: string; readonly server: Bun.Server }> {
  const payload = "y".repeat(768);
  let server: Bun.Server;
  server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    routes: {
      "/.well-known/agent-card.json": () => Response.json({
        name: "Noisy Worker",
        description: "Streams large A2A artifacts",
        protocolVersion: "0.3.0",
        version: "0.1.0",
        url: `http://127.0.0.1:${server.port}/a2a/jsonrpc`,
        capabilities: { streaming: true, pushNotifications: false },
        defaultInputModes: ["text"],
        defaultOutputModes: ["text"],
        skills: [],
      }),
      "/a2a/jsonrpc": () => new Response(
        new ReadableStream<Uint8Array>({
          start(controller) {
            const encoder = new TextEncoder();
            for (let index = 0; index < 6000; index += 1) {
              controller.enqueue(encoder.encode("event: message\n"));
              controller.enqueue(encoder.encode(`data: ${JSON.stringify({
                result: {
                  kind: "artifact-update",
                  taskId: "task-1",
                  contextId: "ctx-1",
                  artifact: {
                    parts: [{ kind: "text", text: `artifact-${index}-${payload}` }],
                  },
                },
              })}\n\n`));
            }
            controller.enqueue(encoder.encode("event: message\n"));
            controller.enqueue(encoder.encode(`data: ${JSON.stringify({
              result: {
                kind: "artifact-update",
                taskId: "task-1",
                contextId: "ctx-1",
                artifact: {
                  parts: [{ kind: "text", text: "final artifact tail" }],
                },
              },
            })}\n\n`));
            controller.enqueue(encoder.encode("event: message\n"));
            controller.enqueue(encoder.encode(`data: ${JSON.stringify({
              result: {
                kind: "status-update",
                taskId: "task-1",
                contextId: "ctx-1",
                status: { state: "completed" },
              },
            })}\n\n`));
            controller.close();
          },
        }),
        {
          headers: { "content-type": "text/event-stream" },
        },
      ),
    },
  });

  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    server,
  };
}

async function writeA2aWorkerConfig(cwd: string, baseUrl: string): Promise<void> {
  await writeSkill(cwd);
  await writeFile(
    join(cwd, ".maestro", "config.yaml"),
    [
      "execution:",
      "  defaultWorker: noisy-worker",
      "  allowA2a: true",
      "workers:",
      "  noisy-worker:",
      "    enabled: true",
      "    transport: a2a",
      `    url: ${baseUrl}`,
    ].join("\n"),
  );
}

async function readOnlyExecution(cwd: string, missionId: string) {
  const store = new FsExecutionStoreAdapter(cwd);
  const records = await store.list(missionId);
  expect(records).toHaveLength(1);
  return records[0]!;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-output-memory-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await a2aServer?.stop(true);
  a2aServer = undefined;
  await rm(tmpDir, { recursive: true, force: true });
});

describe("worker output memory integration", () => {
  it("keeps CLI feature runs memory-bounded and truncates oversized execution records", async () => {
    const missionId = await createMission(tmpDir);
    await writeCliWorkerConfig(tmpDir);

    const result = await runWithRssSampling(
      ["feature", "run", "--mission", missionId, "--worker", "noisy-worker"],
      tmpDir,
    );

    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
    expect(result.rssSamplesKb.length).toBeGreaterThan(1);
    expect(peakGrowthMb(result.rssSamplesKb)).toBeLessThan(220);

    const execution = await readOnlyExecution(tmpDir, missionId);
    expect(execution.stdoutRaw).toContain("[truncated");
    expect(execution.stdoutRaw).toContain("first visible line");
    expect(execution.stdoutRaw).toContain("final visible tail");
    expect(execution.stdoutRaw.length).toBeLessThan(140_000);

    const runtimeEventStore = new FsRuntimeEventStoreAdapter(tmpDir);
    const tail = await runtimeEventStore.tailByFeature(missionId, "f1");
    expect(tail.some((event) => event.text?.includes("final visible tail"))).toBe(true);

    const snapshot = await run(
      ["mission-control", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(snapshot.exitCode).toBe(0);
    expect(snapshot.stdout).toContain("final visible tail");
  }, 20_000);

  it("keeps A2A feature runs memory-bounded and truncates oversized SSE transcripts", async () => {
    const missionId = await createMission(tmpDir);
    const server = await startNoisyA2aServer();
    a2aServer = server.server;
    await writeA2aWorkerConfig(tmpDir, server.baseUrl);

    const result = await runWithRssSampling(
      ["feature", "run", "--mission", missionId, "--worker", "noisy-worker"],
      tmpDir,
    );

    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
    expect(result.rssSamplesKb.length).toBeGreaterThan(1);
    expect(peakGrowthMb(result.rssSamplesKb)).toBeLessThan(220);

    const execution = await readOnlyExecution(tmpDir, missionId);
    expect(execution.stdoutRaw).toContain("[truncated");
    expect(execution.stdoutRaw.length).toBeLessThan(140_000);

    const runtimeEventStore = new FsRuntimeEventStoreAdapter(tmpDir);
    const tail = await runtimeEventStore.tailByFeature(missionId, "f1");
    expect(tail.some((event) => event.text?.includes("final artifact tail"))).toBe(true);

    const snapshot = await run(
      ["mission-control", "--mission", missionId, "--json"],
      tmpDir,
    );
    expect(snapshot.exitCode).toBe(0);
    expect(snapshot.stdout).toContain("final artifact tail");
  }, 20_000);
});
