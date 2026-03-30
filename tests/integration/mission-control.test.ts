/**
 * Integration tests for mission-control command
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { enterAltScreen, exitAltScreen } from "../../src/tui/terminal/ansi.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];
const DIST_CLI = join(import.meta.dir, "..", "..", "dist", "maestro");

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 15_000;
const PTY_TIMEOUT_MS = 30_000;
let pythonAvailable = false;

const PYTHON_PTY_RUNNER = `
import base64, json, os, select, signal, subprocess, sys, time, pty

payload = json.loads(sys.argv[1])
master, slave = pty.openpty()
proc = subprocess.Popen(
    payload["cmd"],
    cwd=payload["cwd"],
    stdin=slave,
    stdout=slave,
    stderr=slave,
    close_fds=True,
)
os.close(slave)

input_data = base64.b64decode(payload["input"])
delay_s = payload.get("delayMs", 250) / 1000.0
deadline = time.time() + (payload.get("timeoutMs", 30000) / 1000.0)
send_at = time.time() + delay_s
first_output_seen = False
send_attempts = 0
next_send_at = send_at
chunks = []
timed_out = False

while True:
    now = time.time()
    timeout = max(0.0, min(0.1, deadline - now))
    readable, _, _ = select.select([master], [], [], timeout)
    if readable:
        try:
            data = os.read(master, 4096)
            if data:
                chunks.append(data)
                first_output_seen = True
        except OSError:
            pass

    if first_output_seen and send_attempts < 3 and now >= next_send_at:
        os.write(master, input_data)
        send_attempts += 1
        next_send_at = now + 0.5

    if proc.poll() is not None:
        end = time.time() + 0.2
        while time.time() < end:
            try:
                data = os.read(master, 4096)
                if not data:
                    break
                chunks.append(data)
            except OSError:
                break
        break

    if time.time() >= deadline:
        timed_out = True
        proc.kill()
        proc.wait()
        break

os.close(master)
result = {
    "exitCode": proc.returncode,
    "signal": signal.Signals(-proc.returncode).name if proc.returncode is not None and proc.returncode < 0 else None,
    "timedOut": timed_out,
    "rawOutput": base64.b64encode(b"".join(chunks)).decode("ascii"),
}
print(json.dumps(result))
`;

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

async function runCompiled(
  args: string[],
  cwd = process.cwd(),
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([DIST_CLI, ...args], {
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

async function commandExists(command: string): Promise<boolean> {
  const proc = Bun.spawn(["/bin/zsh", "-lc", `command -v ${command}`], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return (await proc.exited) === 0;
}

function stripAnsi(text: string): string {
  return text
    .replace(/\u001b\[[0-9;?]*[ -/]*[@-~]/g, "")
    .replace(/\r/g, "")
    .replace(/\u0008/g, "")
    .replace(/[^\S\n]+\n/g, "\n")
    .trim();
}

interface PtyRunOptions {
  readonly input: string;
  readonly delayMs?: number;
  readonly timeoutMs?: number;
}

interface PtyRunResult {
  readonly rawOutput: string;
  readonly plainOutput: string;
  readonly exitCode: number | null;
  readonly signal: string | null;
  readonly timedOut: boolean;
}

async function runCompiledInteractivePty(
  cwd: string,
  args: string[],
  opts: PtyRunOptions,
): Promise<PtyRunResult> {
  const payload = JSON.stringify({
    cmd: [DIST_CLI, ...args],
    cwd,
    input: Buffer.from(opts.input, "utf8").toString("base64"),
    delayMs: opts.delayMs ?? 250,
    timeoutMs: opts.timeoutMs ?? PTY_TIMEOUT_MS,
  });

  const proc = Bun.spawn(["python3", "-c", PYTHON_PTY_RUNNER, payload], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  expect(exitCode).toBe(0);
  expect(stderr.trim()).toBe("");
  const result = JSON.parse(stdout) as {
    exitCode: number | null;
    signal: string | null;
    timedOut: boolean;
    rawOutput: string;
  };
  const rawOutput = Buffer.from(result.rawOutput, "base64").toString("utf8");
  return {
    rawOutput,
    plainOutput: stripAnsi(rawOutput),
    exitCode: result.exitCode,
    signal: result.signal,
    timedOut: result.timedOut,
  };
}

function expectCleanPtyExit(result: PtyRunResult): void {
  expect(result.timedOut).toBe(false);
  expect(result.exitCode).toBe(0);
  expect(result.signal).toBeNull();
  expect(result.rawOutput).toContain(enterAltScreen);
  expect(result.rawOutput).toContain(exitAltScreen);
}

beforeAll(async () => {
  pythonAvailable = await commandExists("python3");
  const build = Bun.spawn(["bun", "run", "build"], {
    cwd: join(import.meta.dir, "..", ".."),
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(await build.exited).toBe(0);
});

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
    expect(stdout).toContain("┌");
    expect(stdout).toContain("┘");
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

  it("rejects interactive mode without both tty streams", async () => {
    const missionId = await createMission(tmpDir);

    const { stdout, stderr, exitCode } = await run(
      ["mission-control", "--mission", missionId],
      tmpDir,
    );

    expect(exitCode).toBe(1);
    expect(stdout + stderr).toContain("Interactive mode requires TTY input and output");
  }, SLOW_CLI_TIMEOUT_MS);

  it("compiled binary interactive mode exits cleanly on q", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "q" },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("Mission Control");
    expect(result.plainOutput).toContain("Features");
    expect(result.plainOutput).toContain("┌");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode exits cleanly on Ctrl+T", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "\u0014" },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("Mission Control");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode exits cleanly on Ctrl+C", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "\u0003" },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("Mission Control");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode survives idle polling before quit", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "q", delayMs: 2_500 },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("Progress Log");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode remains stable across repeated launch and quit cycles", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    for (let i = 0; i < 3; i++) {
      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        { input: "q" },
      );
      expectCleanPtyExit(result);
    }
  }, PTY_TIMEOUT_MS);
});
