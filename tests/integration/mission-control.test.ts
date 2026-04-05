/**
 * Integration tests for mission-control command
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsFeatureStoreAdapter } from "../../src/adapters/feature-store.adapter.js";
import { FsMissionStoreAdapter } from "../../src/adapters/mission-store.adapter.js";
import { FsRuntimeEventStoreAdapter } from "../../src/adapters/runtime-event-store.adapter.js";
import { FsRuntimeStoreAdapter } from "../../src/adapters/runtime-store.adapter.js";
import { buildModalOptions } from "../../src/tui/app/modal-builders.js";
import { computeScreenLayout } from "../../src/tui-opentui/components/builders.js";
import { createInitialState, reduce } from "../../src/tui/state/reducer.js";
import { enterAltScreen, exitAltScreen } from "../../src/tui/terminal/ansi.js";
import type { MissionControlSnapshot } from "../../src/tui/state/types.js";
import { layoutModal } from "../../src/tui/widgets/modal.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];
const DIST_CLI = join(import.meta.dir, "..", "..", "dist", "maestro");
const CTRL_P = "\u0010";

let tmpDir: string;
const SLOW_CLI_TIMEOUT_MS = 15_000;
const PTY_TIMEOUT_MS = 30_000;
let pythonAvailable = false;
const ANSI_PATTERN = /\u001B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~]|\][^\u0007\u001B]*(?:\u0007|\u001B\\))/g;

const PYTHON_PTY_RUNNER = `
import base64, json, os, select, signal, subprocess, sys, time, pty, fcntl, termios, struct

payload = json.loads(sys.argv[1])
master, slave = pty.openpty()
rows = payload.get("rows", 24)
cols = payload.get("cols", 80)
winsize = struct.pack("HHHH", rows, cols, 0, 0)
fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)
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
wait_for_text = payload.get("waitForText")
input_steps = [
    {
        "data": base64.b64decode(step["input"]),
        "delay_s": step.get("delayMs", 0) / 1000.0,
    }
    for step in payload.get("inputSteps", [])
]
first_output_seen = False
send_attempts = 0
send_at = None
next_send_at = None
next_step_index = 0
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
                if send_at is None:
                    if wait_for_text:
                        current_output = b"".join(chunks).decode("utf8", "replace")
                        if wait_for_text in current_output:
                            send_at = time.time()
                            next_send_at = send_at + (input_steps[0]["delay_s"] if input_steps else delay_s)
                    else:
                        send_at = time.time()
                        next_send_at = send_at + (input_steps[0]["delay_s"] if input_steps else delay_s)
        except OSError:
            pass

    if first_output_seen and send_at is not None and next_send_at is not None and now >= next_send_at:
        if input_steps:
            os.write(master, input_steps[next_step_index]["data"])
            next_step_index += 1
            if next_step_index < len(input_steps):
                next_send_at = now + input_steps[next_step_index]["delay_s"]
            else:
                next_send_at = None
        elif send_attempts < 3:
            os.write(master, input_data)
            send_attempts += 1
            next_send_at = now + 0.5
        else:
            next_send_at = None

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

async function setMissionStatus(
  cwd: string,
  missionId: string,
  status: "approved" | "executing" | "paused",
): Promise<void> {
  const args = status === "approved"
    ? ["mission", "approve", missionId, "--json"]
    : ["mission", "update", missionId, "--status", status, "--json"];
  const { exitCode } = await run(args, cwd);
  expect(exitCode).toBe(0);
}

async function listFeatureStatuses(
  cwd: string,
  missionId: string,
): Promise<Record<string, string>> {
  const { stdout, exitCode } = await run(
    ["feature", "list", "--mission", missionId, "--json"],
    cwd,
  );
  expect(exitCode).toBe(0);
  const payload = JSON.parse(stdout) as {
    features: Array<{ id: string; status: string }>;
  };
  return Object.fromEntries(payload.features.map((feature) => [feature.id, feature.status]));
}

async function setFeatureStatus(
  cwd: string,
  missionId: string,
  featureId: string,
  status: "assigned" | "in-progress" | "review" | "blocked" | "done" | "pending",
): Promise<void> {
  const { exitCode } = await run(
    ["feature", "update", featureId, "--mission", missionId, "--status", status, "--json"],
    cwd,
  );
  expect(exitCode).toBe(0);
}

async function seedLiveRuntimeOutput(
  cwd: string,
  missionId: string,
  featureId: string,
): Promise<void> {
  const runtimeStore = new FsRuntimeStoreAdapter(cwd);
  const runtimeEventStore = new FsRuntimeEventStoreAdapter(cwd);
  const startedAt = new Date(Date.now() - 15_000).toISOString();
  const lastSeenAt = new Date(Date.now() - 2_000).toISOString();
  const leaseExpiresAt = new Date(Date.now() + 60_000).toISOString();
  const stdoutAt = new Date(Date.now() - 3_000).toISOString();
  const stderrAt = new Date(Date.now() - 1_000).toISOString();
  await runtimeStore.save(missionId, featureId, {
    featureId,
    attemptId: "attempt-live-output",
    attempt: 1,
    agent: "codex",
    sessionId: "5634c102-9871-4001-86f8-89399077624e",
    runtimeState: "live",
    startedAt,
    lastSeenAt,
    leaseExpiresAt,
    recoveryMetadata: {
      retryCount: 0,
      history: [],
    },
  });
  await runtimeEventStore.append(missionId, {
    id: "event-1",
      missionId,
      featureId,
      attemptId: "attempt-live-output",
      worker: "codex",
      timestamp: stdoutAt,
      kind: "stdout",
      text: "Reading runtime-supervision.usecase.ts",
    });
  await runtimeEventStore.append(missionId, {
    id: "event-2",
      missionId,
      featureId,
      attemptId: "attempt-live-output",
      worker: "codex",
      timestamp: stderrAt,
      kind: "stderr",
      text: "Retry budget still available",
      });
}

async function seedLiveRuntime(
  cwd: string,
  missionId: string,
  featureId: string,
): Promise<void> {
  const runtimeStore = new FsRuntimeStoreAdapter(cwd);
  const startedAt = new Date(Date.now() - 15_000).toISOString();
  const lastSeenAt = new Date(Date.now() - 2_000).toISOString();
  const leaseExpiresAt = new Date(Date.now() + 60_000).toISOString();
  await runtimeStore.save(missionId, featureId, {
    featureId,
    attemptId: "attempt-live-only",
    attempt: 1,
    agent: "codex",
    sessionId: "5634c102-9871-4001-86f8-89399077624e",
    runtimeState: "live",
    startedAt,
    lastSeenAt,
    leaseExpiresAt,
    recoveryMetadata: {
      retryCount: 0,
      history: [],
    },
    });
}

async function setFeatureToReview(
  cwd: string,
  missionId: string,
  featureId: string,
): Promise<void> {
  await setFeatureStatus(cwd, missionId, featureId, "assigned");
  await setFeatureStatus(cwd, missionId, featureId, "in-progress");
  await setFeatureStatus(cwd, missionId, featureId, "review");
}

async function createPendingHandoff(cwd: string): Promise<string> {
  const { stdout, exitCode } = await run([
    "handoff",
    "--skip-session",
    "--sitrep",
    "preview sitrep",
    "--quickstart",
    "preview command",
    "--json",
  ], cwd);
  expect(exitCode).toBe(0);
  return JSON.parse(stdout).id;
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
    .replace(ANSI_PATTERN, "")
    .replace(/\r/g, "")
    .replace(/\u0008/g, "")
    .replace(/[^\S\n]+\n/g, "\n")
    .trim();
}

function hasAnimatedHeaderFrame(plainOutput: string): boolean {
  return plainOutput.includes("•●•") || plainOutput.includes("••●");
}

function getDurationSamples(plainOutput: string): string[] {
  const matches = plainOutput.matchAll(/TIME\s+((?:\d+[hms]\s*)+)/g);
  return [...new Set(
    Array.from(matches, (match) => match[1]?.replace(/\s+/g, " ").trim() ?? "")
      .filter((sample) => sample.length > 0),
  )];
}

interface PtyRunOptions {
  readonly input: string;
  readonly inputSteps?: ReadonlyArray<{ chars: string; delayMs?: number }>;
  readonly delayMs?: number;
  readonly timeoutMs?: number;
  readonly waitForText?: string;
  readonly rows?: number;
  readonly cols?: number;
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
    inputSteps: opts.inputSteps?.map((step) => ({
      input: Buffer.from(step.chars, "utf8").toString("base64"),
      delayMs: step.delayMs ?? 0,
    })),
    delayMs: opts.delayMs ?? 250,
    timeoutMs: opts.timeoutMs ?? PTY_TIMEOUT_MS,
    waitForText: opts.waitForText,
    rows: opts.rows ?? 24,
    cols: opts.cols ?? 80,
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

function expectConfigOverlay(output: string): void {
  expect(output).toContain("Config");
  expect(output).toContain("[overview]");
  expect(output).toContain("workers");
  expect(output).toContain("project config");
}

function expectWorkersOverlay(output: string): void {
  expect(output).toContain("Workers");
  expect(output).toContain("Claude Code");
  expect(output).toContain("configured; not checked in read-only mode");
}

function expectRuntimeOverlay(output: string): void {
  expect(output).toContain("Runtime");
  expect(output).toContain("features right now");
  expect(output).toContain("No runtime item selected");
}

function encodeLeftClick(x: number, y: number): string {
  return `\u001b[<0;${x + 1};${y + 1}M`;
}

function getMissionControlModalParentRect(width: number, height: number) {
  const layout = computeScreenLayout(width, height, { mode: "mission" } as never);
  return {
    x: Math.max(1, Math.floor((layout.innerWidth - layout.modalWidth) / 2)),
    y: Math.max(1, Math.floor((layout.innerHeight - layout.modalHeight) / 2)),
    width: layout.modalWidth,
    height: layout.modalHeight,
  };
}

function getFeatureActionMouseClicks(optionIndex: number, width = 80, height = 24): {
  readonly selectClick: string;
  readonly confirmClick: string;
} {
  const parentRect = getMissionControlModalParentRect(width, height);
  const snapshot: MissionControlSnapshot = {
    mode: "mission",
    missionId: "mission-1",
    missionTitle: "Mission 1",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 1, active: 1 },
    statusProgress: {
      completed: 0,
      total: 1,
      inFlight: 1,
      blocked: 0,
      queued: 0,
      completionPct: 0,
    },
    tokenCounters: null,
    features: [{
      id: "f1",
      title: "Feature 1",
      status: "pending",
      milestoneId: "m1",
      workerType: "backend",
      hasReport: false,
    }],
    milestones: [],
    missionOverview: null,
    activeFeature: null,
    taskPreviews: [],
    activeWorker: null,
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    configInspector: null,
    workerHealth: [],
    runtimeProcesses: [],
    progressLog: [],
    canPause: false,
    canResume: false,
    home: null,
  };
  const selectingState = reduce(createInitialState(snapshot), { type: "enter" });
  const selectingModal = buildModalOptions(selectingState);
  if (!selectingModal) {
    throw new Error("Expected feature-action modal options for selecting state");
  }
  const selectingLayout = layoutModal(parentRect, selectingModal);
  const confirmingState = reduce(selectingState, { type: "modal-select", option: optionIndex });
  const confirmingModal = buildModalOptions(confirmingState);
  if (!confirmingModal) {
    throw new Error("Expected feature-action modal options for confirming state");
  }
  const confirmingLayout = layoutModal(parentRect, confirmingModal);
  const selectRect = selectingLayout.itemRects[optionIndex]!;
  const confirmRect = confirmingLayout.itemRects[optionIndex]!;
  return {
    selectClick: encodeLeftClick(selectRect.x + 1, selectRect.y),
    confirmClick: encodeLeftClick(confirmRect.x + 1, confirmRect.y),
  };
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
    await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

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
    expect(snapshot.statusProgress).toBeDefined();
    expect(snapshot.statusProgress.total).toBe(2);
    expect(snapshot.statusProgress.completed).toBe(0);
    expect(snapshot.statusProgress.queued).toBe(1);
    expect(snapshot.statusProgress.inFlight).toBe(1);
    expect(snapshot.features).toBeDefined();
    expect(Array.isArray(snapshot.features)).toBe(true);
    expect(snapshot.features.length).toBe(2);
    expect(snapshot.session).toBeDefined();
    expect(snapshot.session.branch).toBe("main");
    expect(snapshot.session.workingTreeClean).toBe(false);
    expect(Array.isArray(snapshot.session.changedFiles)).toBe(true);
    expect(snapshot.session.changedFiles.length).toBeGreaterThan(0);
    expect(typeof snapshot.session.diffStat).toBe("string");
    expect(snapshot.session.diffStat.length).toBeGreaterThan(0);
    expect(Array.isArray(snapshot.pendingHandoffs)).toBe(true);
    expect(snapshot.configSummary).toBeDefined();
    expect(snapshot.configSummary.missionDirectory).toBe(`.maestro/missions/${missionId}`);
    expect(snapshot.configSummary.workerTypes).toContain("test-skill");
      expect(Array.isArray(snapshot.configSummary.checks)).toBe(true);
      expect(Array.isArray(snapshot.runtimeProcesses)).toBe(true);
      expect(snapshot.runtimeProcesses.length).toBe(0);
      expect(snapshot.activeWorker).toBeNull();
      expect(snapshot.progressLog).toBeDefined();
      expect(snapshot.milestones).toBeDefined();
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview returns non-empty dashboard text containing mission title", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview"],
        tmpDir,
      );

    expect(exitCode).toBe(0);
    expect(stdout.length).toBeGreaterThan(0);
    expect(stdout).toContain("Mission Control");
    expect(stdout).toContain("Tasks");
    expect(stdout).toContain("┌");
      expect(stdout).toContain("┘");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview features renders the task browser", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "features"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Tasks");
      expect(stdout).toContain("Select a task to focus");
      expect(stdout).toContain("Feature 1");
      expect(stdout).toContain("Feature 2");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview dependencies targets the requested feature", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "dependencies", "--feature", "f2"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Dependencies");
      expect(stdout).toContain("Feature 2");
    }, SLOW_CLI_TIMEOUT_MS);

      it("--preview handoffs renders the handoffs modal", async () => {
        const handoffId = await createPendingHandoff(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--preview", "handoffs", "--handoff", handoffId],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Handoffs");
      expect(stdout).toContain(handoffId);
        expect(stdout).toContain("Details hidden in read-only output");
      }, SLOW_CLI_TIMEOUT_MS);

      it("--preview handoff accepts the singular alias", async () => {
        const handoffId = await createPendingHandoff(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--preview", "handoff", "--handoff", handoffId],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expect(stdout).toContain("Handoffs");
        expect(stdout).toContain(handoffId);
      }, SLOW_CLI_TIMEOUT_MS);

        it("--preview config renders the config modal", async () => {
          const missionId = await createMission(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--preview", "config"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expectConfigOverlay(stdout);
      }, SLOW_CLI_TIMEOUT_MS);

      it("--preview feat accepts the feature browser shorthand", async () => {
        const missionId = await createMission(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--preview", "feat"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expect(stdout).toContain("Tasks");
        expect(stdout).toContain("Feature 1");
      }, SLOW_CLI_TIMEOUT_MS);

      it("--preview cfg accepts the config shorthand", async () => {
        const missionId = await createMission(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--preview", "cfg"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expectConfigOverlay(stdout);
      }, SLOW_CLI_TIMEOUT_MS);

    it("--preview runtime renders the runtime modal", async () => {
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      await seedLiveRuntime(tmpDir, missionId, "f1");

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "runtime"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Runtime");
      expect(stdout).toContain("Feature 1");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview workers renders the worker health modal", async () => {
      const missionId = await createMission(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--preview", "workers"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expectWorkersOverlay(stdout);
      }, SLOW_CLI_TIMEOUT_MS);

    it("--preview output shows the empty state when no runtime output is captured yet", async () => {
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      await seedLiveRuntime(tmpDir, missionId, "f1");

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "output", "--feature", "f1"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Worker Output");
      expect(stdout).toContain("No runtime output captured yet.");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview workers renders the workers modal", async () => {
      const missionId = await createMission(tmpDir);

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--preview", "workers"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        expectWorkersOverlay(stdout);
      }, SLOW_CLI_TIMEOUT_MS);

    it("--preview worker accepts the singular alias", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "worker"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Workers");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview output renders streamed runtime lines for the selected feature", async () => {
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      await seedLiveRuntimeOutput(tmpDir, missionId, "f1");

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "output", "--feature", "f1"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Worker Output");
      expect(stdout).toContain("Reading runtime-supervision.usecase.ts");
      expect(stdout).toContain("Retry budget still available");
      }, SLOW_CLI_TIMEOUT_MS);

      it("--json keeps assigned, in-progress, and review features invisible to runtime views until a real lease exists", async () => {
        const statuses = ["assigned", "in-progress", "review"] as const;

        for (const status of statuses) {
          const missionId = await createMission(tmpDir);

          if (status === "assigned") {
            await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
          } else if (status === "in-progress") {
            await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
            await setFeatureStatus(tmpDir, missionId, "f1", "in-progress");
          } else {
            await setFeatureToReview(tmpDir, missionId, "f1");
          }

          const { stdout, exitCode } = await run(
            ["mission-control", "--mission", missionId, "--json"],
            tmpDir,
          );

          expect(exitCode).toBe(0);
          const snapshot = JSON.parse(stdout);
          expect(snapshot.activeWorker).toBeNull();
          expect(snapshot.runtimeProcesses).toEqual([]);

          const preview = await run(
            ["mission-control", "--mission", missionId, "--preview", "runtime"],
            tmpDir,
          );
          expect(preview.exitCode).toBe(0);
          expectRuntimeOverlay(preview.stdout);
        }
      }, SLOW_CLI_TIMEOUT_MS);

      it("--json and default preview follow the real runtime when an earlier active feature has none", async () => {
        const missionId = await createMission(tmpDir);
        await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
        await setFeatureStatus(tmpDir, missionId, "f2", "assigned");
        await setFeatureStatus(tmpDir, missionId, "f2", "in-progress");
        await seedLiveRuntime(tmpDir, missionId, "f2");

        const { stdout, exitCode } = await run(
          ["mission-control", "--mission", missionId, "--json"],
          tmpDir,
        );

        expect(exitCode).toBe(0);
        const snapshot = JSON.parse(stdout);
        expect(snapshot.activeWorker).toMatchObject({
          featureId: "f2",
          featureTitle: "Feature 2",
          runtimeState: "live",
          agent: "codex",
          sessionId: "5634c102-9871-4001-86f8-89399077624e",
        });
        expect(snapshot.session).toMatchObject({
          agent: "codex",
          sessionId: "5634c102-9871-4001-86f8-89399077624e",
        });
        expect(snapshot.runtimeProcesses).toHaveLength(1);
        expect(snapshot.runtimeProcesses[0]).toMatchObject({
          featureId: "f2",
          runtimeState: "live",
          agent: "codex",
        });

        const preview = await run(
          ["mission-control", "--mission", missionId, "--preview"],
          tmpDir,
        );
        expect(preview.exitCode).toBe(0);
        expect(preview.stdout).toContain("agent      codex");
        expect(preview.stdout).toContain("session    5634c102");
      }, SLOW_CLI_TIMEOUT_MS);

      it("--preview out accepts the runtime output alias", async () => {
        const missionId = await createMission(tmpDir);
        await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
        await seedLiveRuntimeOutput(tmpDir, missionId, "f1");

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "out", "--feature", "f1"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Worker Output");
    }, SLOW_CLI_TIMEOUT_MS);

    it("--json projects failed runtime state without auto-requeue", async () => {
      const missionId = await createMission(tmpDir);
      await setMissionStatus(tmpDir, missionId, "approved");
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      const runtimeStore = new FsRuntimeStoreAdapter(tmpDir);

      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-1",
      attempt: 1,
      agent: "unknown",
      runtimeState: "live",
      startedAt: "2026-04-01T00:00:00.000Z",
      lastSeenAt: "2026-04-01T00:00:00.000Z",
      leaseExpiresAt: "2026-04-01T00:01:00.000Z",
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      const snapshot = JSON.parse(stdout);
      expect(snapshot.runtimeProcesses.find((process: { featureId: string }) => process.featureId === "f1")).toMatchObject({
        runtimeState: "failed",
      });
      expect(snapshot.activeWorker).toMatchObject({
        featureId: "f1",
        runtimeState: "failed",
      });
      expect(await listFeatureStatuses(tmpDir, missionId)).toMatchObject({ f1: "assigned" });
      expect(await runtimeStore.get(missionId, "f1")).toMatchObject({
        runtimeState: "live",
        recoveryMetadata: { retryCount: 0, history: [] },
      });
    }, SLOW_CLI_TIMEOUT_MS);

    it("--preview keeps runtime storage read-only", async () => {
      const missionId = await createMission(tmpDir);
      await setMissionStatus(tmpDir, missionId, "approved");
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      const runtimeStore = new FsRuntimeStoreAdapter(tmpDir);

      await runtimeStore.save(missionId, "f1", {
        featureId: "f1",
        attemptId: "attempt-1",
        attempt: 1,
        agent: "unknown",
        runtimeState: "live",
        startedAt: "2026-04-01T00:00:00.000Z",
        lastSeenAt: "2026-04-01T00:00:00.000Z",
        leaseExpiresAt: "2026-04-01T00:01:00.000Z",
        recoveryMetadata: {
          retryCount: 0,
          history: [],
        },
      });

      const { stdout, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
      expect(stdout).toContain("Mission Control");
      expect(stdout).toContain("Feature 1");
      expect(await listFeatureStatuses(tmpDir, missionId)).toMatchObject({ f1: "assigned" });
      expect(await runtimeStore.get(missionId, "f1")).toMatchObject({
        runtimeState: "live",
        recoveryMetadata: { retryCount: 0, history: [] },
      });
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

    it("errors for an unknown preview feature selector", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--preview", "dependencies", "--feature", "f9"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      expect(stdout + stderr).toContain("Feature f9 not found");
    }, SLOW_CLI_TIMEOUT_MS);

    it("errors for an unknown preview handoff selector", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["mission-control", "--preview", "handoffs", "--handoff", "handoff-missing"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      expect(stdout + stderr).toContain("Handoff handoff-missing not found");
    }, SLOW_CLI_TIMEOUT_MS);

    it("returns home mode when no missions exist in a git repo", async () => {
      const { stdout, exitCode } = await run(
        ["mission-control", "--json"],
      tmpDir,
    );

    expect(exitCode).toBe(0);
    const snapshot = JSON.parse(stdout);
      expect(snapshot.mode).toBe("home");
      expect(snapshot.home.headline).toBe("No missions yet");
      expect(snapshot.home.actions.length).toBeGreaterThan(0);
    }, SLOW_CLI_TIMEOUT_MS);

    it("redacts pending handoff details in read-only json output", async () => {
      const handoffId = await createPendingHandoff(tmpDir);

      const { stdout, exitCode } = await run(
        ["mission-control", "--json"],
        tmpDir,
      );

      expect(exitCode).toBe(0);
        const snapshot = JSON.parse(stdout);
        expect(snapshot.pendingHandoffs).toEqual([
          expect.objectContaining({
            id: handoffId,
            agent: expect.any(String),
            message: "Details hidden in read-only output",
          }),
        ]);
        expect(snapshot.home.pendingHandoffs).toEqual(snapshot.pendingHandoffs);
      }, SLOW_CLI_TIMEOUT_MS);

    it("renders a guided home frame when no missions exist in a git repo", async () => {
      const { stdout, exitCode } = await run(
        ["mission-control", "--preview"],
        tmpDir,
      );

    expect(exitCode).toBe(0);
    expect(stdout).toContain("HOME");
      expect(stdout).toContain("Environment");
      expect(stdout).toContain("Pending Handoffs");
    }, SLOW_CLI_TIMEOUT_MS);

    it("renders the overview modal for home features previews", async () => {
      const { stdout, exitCode } = await run(
        ["mission-control", "--preview", "features"],
        tmpDir,
      );

        expect(exitCode).toBe(0);
        expect(stdout).toContain("Overview");
        expect(stdout).toContain("Environment");
        expect(stdout).toContain("maestro init");
        expect(stdout).toContain("maestro doctor");
      }, SLOW_CLI_TIMEOUT_MS);

    it("errors for dependencies previews in home mode", async () => {
      const { stdout, stderr, exitCode } = await run(
        ["mission-control", "--preview", "dependencies"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
      expect(stdout + stderr).toContain("Dependencies preview requires a mission");
    }, SLOW_CLI_TIMEOUT_MS);

  it("returns home mode outside a git repo", async () => {
    const outsideDir = await mkdtemp(join(tmpdir(), "maestro-mc-home-"));
    try {
      const { stdout, exitCode } = await run(
        ["mission-control", "--json"],
        outsideDir,
      );

      expect(exitCode).toBe(0);
      const snapshot = JSON.parse(stdout);
      expect(snapshot.mode).toBe("home");
      expect(snapshot.home.headline).toBe("No project detected");
    } finally {
      await rm(outsideDir, { recursive: true, force: true });
    }
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects interactive mode without both tty streams", async () => {
    const missionId = await createMission(tmpDir);

    const { stdout, stderr, exitCode } = await run(
      ["mission-control", "--mission", missionId],
      tmpDir,
    );

      expect(exitCode).toBe(1);
      expect(stdout + stderr).toContain("Interactive mode requires TTY input and output");
      expect(stdout + stderr).toContain("Use --preview");
    }, SLOW_CLI_TIMEOUT_MS);

    it("rejects --once with migration guidance", async () => {
      const missionId = await createMission(tmpDir);

      const { stdout, stderr, exitCode } = await run(
        ["mission-control", "--mission", missionId, "--once"],
        tmpDir,
      );

      expect(exitCode).toBe(1);
    expect(stdout + stderr).toContain("--once");
    expect(stdout + stderr).toContain("--preview");
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
    expect(result.plainOutput).toContain("Tasks");
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
    expect(result.plainOutput).toContain("Timeline");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode animates header dots while an executing mission idles", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    await setMissionStatus(tmpDir, missionId, "approved");
    await setMissionStatus(tmpDir, missionId, "executing");

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "q", delayMs: 1_250, waitForText: "Mission Control" },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("●••");
    expect(hasAnimatedHeaderFrame(result.plainOutput)).toBe(true);
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode keeps header dots static while paused", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    await setMissionStatus(tmpDir, missionId, "approved");
    await setMissionStatus(tmpDir, missionId, "executing");
    await setMissionStatus(tmpDir, missionId, "paused");

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "q", delayMs: 450 },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("●••");
    expect(hasAnimatedHeaderFrame(result.plainOutput)).toBe(false);
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

    it("compiled binary interactive mode updates the header time every second while idle", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);
    await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "q", delayMs: 2_300, waitForText: "Mission Control" },
    );

    expectCleanPtyExit(result);
      expect(result.plainOutput).toContain("TIME");
      const durationSamples = getDurationSamples(result.plainOutput);
      expect(durationSamples.length).toBeGreaterThan(1);
    }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode opens and closes the command palette with Ctrl+P", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [
            { chars: CTRL_P, delayMs: 450 },
            { chars: "\u001b", delayMs: 250 },
            { chars: "q", delayMs: 250 },
          ],
          waitForText: "Mission Control",
        },
      );

        expectCleanPtyExit(result);
        expect(result.plainOutput).toContain("Command Palette");
        expect(result.plainOutput).toContain("/ type to filter");
        expect(result.plainOutput).toContain("Browse mission tasks");
        expect(result.plainOutput).not.toContain("Enter open · Esc close");
      }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode filters the command palette and activates Features", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [
            { chars: CTRL_P, delayMs: 450 },
            { chars: "fea", delayMs: 180 },
            { chars: "\r", delayMs: 180 },
            { chars: "\u001b", delayMs: 250 },
            { chars: "q", delayMs: 250 },
          ],
          waitForText: "Mission Control",
        },
      );

        expectCleanPtyExit(result);
        expect(result.plainOutput).toContain("Command Palette");
        expect(result.plainOutput).toContain("Select a task to focus");
        expect(result.plainOutput).toContain("Feature 1");
      }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode filters the command palette and activates Handoff", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [
            { chars: CTRL_P, delayMs: 450 },
            { chars: "hand", delayMs: 180 },
            { chars: "\r", delayMs: 180 },
            { chars: "\u001b", delayMs: 250 },
            { chars: "q", delayMs: 250 },
          ],
          waitForText: "Mission Control",
        },
      );

        expectCleanPtyExit(result);
        expect(result.plainOutput).toContain("Handoffs");
        expect(result.plainOutput).toContain("No pending handoff selected");
      }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode filters the command palette and activates Config", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [
            { chars: CTRL_P, delayMs: 450 },
            { chars: "conf", delayMs: 180 },
            { chars: "\r", delayMs: 180 },
            { chars: "\u001b", delayMs: 250 },
            { chars: "q", delayMs: 250 },
          ],
          waitForText: "Mission Control",
        },
      );

        expectCleanPtyExit(result);
        expectConfigOverlay(result.plainOutput);
      }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode filters the command palette and activates Processes", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [
            { chars: CTRL_P, delayMs: 450 },
            { chars: "proc", delayMs: 180 },
            { chars: "\r", delayMs: 180 },
            { chars: "\u001b", delayMs: 250 },
            { chars: "q", delayMs: 250 },
          ],
          waitForText: "Mission Control",
        },
      );

        expectCleanPtyExit(result);
        expectRuntimeOverlay(result.plainOutput);
      }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode opens the Features overlay", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      {
        input: "",
        inputSteps: [
          { chars: "F", delayMs: 450 },
          { chars: "\u001b", delayMs: 300 },
          { chars: "q", delayMs: 300 },
        ],
        waitForText: "Mission Control",
      },
    );

    expectCleanPtyExit(result);
      expect(result.plainOutput).toContain("Tasks");
      expect(result.plainOutput).toContain("Select a task to focus");
    }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode opens the Handoff overlay", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      {
        input: "",
        inputSteps: [
          { chars: "H", delayMs: 450 },
          { chars: "\u001b", delayMs: 300 },
          { chars: "q", delayMs: 300 },
        ],
        waitForText: "Mission Control",
      },
      );

      expectCleanPtyExit(result);
      expect(result.plainOutput).toContain("Handoffs");
      expect(result.plainOutput).toContain("No pending handoff selected");
    }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode opens the Config overlay", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      {
        input: "",
        inputSteps: [
          { chars: "C", delayMs: 450 },
          { chars: "\u001b", delayMs: 300 },
          { chars: "q", delayMs: 300 },
        ],
        waitForText: "Mission Control",
      },
      );

      expectCleanPtyExit(result);
      expectConfigOverlay(result.plainOutput);
    }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode opens the Processes overlay", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      {
        input: "",
        inputSteps: [
          { chars: "P", delayMs: 450 },
          { chars: "\u001b", delayMs: 300 },
          { chars: "q", delayMs: 300 },
        ],
        waitForText: "Mission Control",
      },
    );

      expectCleanPtyExit(result);
      expectRuntimeOverlay(result.plainOutput);
    }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode follows a real later runtime on startup", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
      await setFeatureStatus(tmpDir, missionId, "f2", "assigned");
      await setFeatureStatus(tmpDir, missionId, "f2", "in-progress");
      await seedLiveRuntime(tmpDir, missionId, "f2");

      const result = await runCompiledInteractivePty(
        tmpDir,
        ["mission-control", "--mission", missionId],
        {
          input: "",
          inputSteps: [{ chars: "q", delayMs: 600 }],
          waitForText: "Mission Control",
        },
        );

        expectCleanPtyExit(result);
        expect(result.plainOutput).toContain("Feature 2");
        expect(result.plainOutput).toContain("in-progress");
      }, PTY_TIMEOUT_MS);

  it("compiled binary interactive mode persists a selected feature transition on keyboard confirm", async () => {
    if (!pythonAvailable) return;
    const missionId = await createMission(tmpDir);

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control", "--mission", missionId],
      { input: "\r\r\r\u001bq" },
    );

    expectCleanPtyExit(result);

    const statuses = await listFeatureStatuses(tmpDir, missionId);
    expect(statuses.f1).toBe("assigned");
  }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode persists a selected feature transition on mouse click confirm", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);
      const clicks = getFeatureActionMouseClicks(0);

        const result = await runCompiledInteractivePty(
          tmpDir,
          ["mission-control", "--mission", missionId],
          {
            input: "",
            inputSteps: [
              { chars: "\r", delayMs: 250 },
              { chars: clicks.selectClick, delayMs: 350 },
              { chars: clicks.confirmClick, delayMs: 300 },
              { chars: "q", delayMs: 250 },
            ],
            waitForText: "Mission Control",
          },
        );

    expectCleanPtyExit(result);

    const statuses = await listFeatureStatuses(tmpDir, missionId);
    expect(statuses.f1).toBe("assigned");
  }, PTY_TIMEOUT_MS);

    it("compiled binary interactive mode closes the modal on outside click without applying a transition", async () => {
      if (!pythonAvailable) return;
      const missionId = await createMission(tmpDir);

        const result = await runCompiledInteractivePty(
          tmpDir,
          ["mission-control", "--mission", missionId],
          {
            input: "",
            inputSteps: [
              { chars: "\r", delayMs: 250 },
              { chars: encodeLeftClick(0, 0), delayMs: 350 },
              { chars: "q", delayMs: 250 },
            ],
            waitForText: "Mission Control",
          },
        );

    expectCleanPtyExit(result);

    const statuses = await listFeatureStatuses(tmpDir, missionId);
    expect(statuses.f1).toBe("pending");
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive home mode exits cleanly on q", async () => {
    if (!pythonAvailable) return;

    const result = await runCompiledInteractivePty(
      tmpDir,
      ["mission-control"],
      { input: "q", delayMs: 450 },
    );

    expectCleanPtyExit(result);
    expect(result.plainOutput).toContain("HOME");
    expect(result.plainOutput).toContain("No missions yet");
    expect(result.plainOutput).toContain("●••");
    expect(hasAnimatedHeaderFrame(result.plainOutput)).toBe(false);
  }, PTY_TIMEOUT_MS);

  it("compiled binary interactive home mode auto-binds to a mission that appears later", async () => {
    if (!pythonAvailable) return;

    const ptyRun = runCompiledInteractivePty(
      tmpDir,
      ["mission-control"],
      {
        input: "",
        inputSteps: [{ chars: "q", delayMs: 2_600 }],
      },
    );

    await Bun.sleep(400);
    const missionId = await createMission(tmpDir);
    await setMissionStatus(tmpDir, missionId, "approved");
    await setMissionStatus(tmpDir, missionId, "executing");
    await setFeatureStatus(tmpDir, missionId, "f1", "assigned");
    await setFeatureStatus(tmpDir, missionId, "f1", "in-progress");
    await seedLiveRuntime(tmpDir, missionId, "f1");

    const result = await ptyRun;

      expectCleanPtyExit(result);
      expect(result.plainOutput).toContain("Mission Control");
      expect(result.plainOutput).toContain("Feature 1");
      expect(result.plainOutput).toContain("RUNNING");
    }, PTY_TIMEOUT_MS);
});
