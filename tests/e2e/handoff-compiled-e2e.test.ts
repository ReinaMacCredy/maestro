import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { access, chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { tmpdir } from "node:os";
import {
  BUILD_TIMEOUT_MS,
  DIST_CLI,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

let tmpDir: string;
let cleanupDirs: string[];

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  cleanupDirs = [];
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-handoff-e2e-"));
  await initGitRepo(tmpDir);
  await runCommand(["git", "config", "user.name", "Test User"], tmpDir);
  await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
  await writeFile(join(tmpDir, "README.md"), "# temp\n");
  await runCommand(["git", "add", "README.md"], tmpDir);
  await runCommand(["git", "commit", "-m", "init"], tmpDir);
});

afterEach(async () => {
  await Promise.all(cleanupDirs.map((dir) => rm(dir, { recursive: true, force: true })));
  await rm(tmpDir, { recursive: true, force: true });
});

async function installFakeProvider(
  name: "codex" | "claude",
  argsPath: string,
  cwdPath: string,
): Promise<string> {
  const binDir = join(tmpDir, "bin");
  await Bun.$`mkdir -p ${binDir}`.quiet();
  const scriptPath = join(binDir, name);
  const script = `#!/bin/sh
printf '%s\n' "$PWD" > "$FAKE_PROVIDER_CWD"
printf '%s\n' "$@" > "$FAKE_PROVIDER_ARGS"
echo "${name} output"
`;
  await writeFile(scriptPath, script);
  await chmod(scriptPath, 0o755);
  return binDir;
}

async function installBehavioralProvider(
  name: "codex" | "claude",
  argsPath: string,
  cwdPath: string,
  promptPath: string,
  opts?: {
    binRoot?: string;
  },
): Promise<string> {
  const binDir = join(opts?.binRoot ?? tmpDir, "bin-behavioral");
  await Bun.$`mkdir -p ${binDir}`.quiet();
  const scriptPath = join(binDir, name);
  const script = `#!/usr/bin/env bun
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const args = process.argv.slice(2);
const prompt = args.at(-1) ?? "";
const env = process.env;

if (env.FAKE_PROVIDER_CWD) {
  writeFileSync(env.FAKE_PROVIDER_CWD, \`\${process.cwd()}\\n\`);
}
if (env.FAKE_PROVIDER_ARGS) {
  writeFileSync(env.FAKE_PROVIDER_ARGS, \`\${args.join("\\n")}\\n\`);
}
if (env.FAKE_PROVIDER_PROMPT) {
  writeFileSync(env.FAKE_PROVIDER_PROMPT, prompt);
}

const handoffId = prompt.match(/maestro handoff pickup --id ([^\\s\`]+)/)?.[1];
const mode = env.FAKE_PROVIDER_MODE;
const maestroBin = env.FAKE_MAESTRO_BIN;
const taskId = env.FAKE_PROVIDER_TASK_ID;

function writeRuntimeFile(relativePath, content) {
  const target = join(process.cwd(), relativePath);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function run(argv) {
  const result = Bun.spawnSync(argv, {
    cwd: process.cwd(),
    stdout: "pipe",
    stderr: "pipe",
    env,
  });
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  return result;
}

if ((mode === "auto-pickup-complete" || mode === "complete-without-pickup") && !maestroBin) {
  console.error("FAKE_MAESTRO_BIN is required");
  process.exit(41);
}

if (env.FAKE_PROVIDER_RUNTIME_NOISE === "1") {
  writeRuntimeFile(".codex/.tmp/plugins.sha", "sha\\n");
  writeRuntimeFile(".codex/.tmp/plugins/README.md", "plugins\\n");
  writeRuntimeFile(".codex/config.toml", "model = \\"gpt-5.4\\"\\n");
  writeRuntimeFile(".codex/installation_id", "installation\\n");
  writeRuntimeFile(".codex/logs_2.sqlite", "logs\\n");
  writeRuntimeFile(".codex/state_5.sqlite", "state\\n");
  writeRuntimeFile(".codex/skills/.system/.codex-system-skills.marker", "marker\\n");
  writeRuntimeFile(".claude/scheduled_tasks.lock", "lock\\n");
  writeRuntimeFile(".maestro/config.yaml", "contracts:\\n  default: prompt\\n");
}

if (env.FAKE_PROVIDER_WORK_FILE) {
  writeRuntimeFile(env.FAKE_PROVIDER_WORK_FILE, env.FAKE_PROVIDER_WORK_CONTENT ?? "provider work\\n");
}

if (mode === "auto-pickup-complete") {
  if (!handoffId) {
    console.error("pickup command missing from prompt");
    process.exit(42);
  }
  const picked = run([maestroBin, "handoff", "pickup", "--id", handoffId, "--json"]);
  if ((picked.exitCode ?? 1) !== 0) {
    process.exit(picked.exitCode ?? 1);
  }
}

if ((mode === "auto-pickup-complete" || mode === "complete-without-pickup") && taskId) {
  const completionArgs = [
    maestroBin,
    "task",
    "update",
    taskId,
    "--status",
    "completed",
    "--reason",
    mode === "auto-pickup-complete" ? "fake provider picked up and finished work" : "fake provider finished work without pickup",
    "--summary",
    mode === "auto-pickup-complete" ? "fake provider completed after pickup" : "fake provider completed without pickup",
    "--json",
  ];
  if (env.FAKE_PROVIDER_VERIFIED_BY) {
    completionArgs.push("--verified-by", env.FAKE_PROVIDER_VERIFIED_BY);
  }
  const completed = run(completionArgs);
  if ((completed.exitCode ?? 1) !== 0) {
    process.exit(completed.exitCode ?? 1);
  }
}

console.log("${name} output");
`;
  await writeFile(scriptPath, script);
  await chmod(scriptPath, 0o755);
  return binDir;
}

async function installFailingProvider(
  name: "codex" | "claude",
): Promise<string> {
  const binDir = join(tmpDir, "bin-fail");
  await Bun.$`mkdir -p ${binDir}`.quiet();
  const scriptPath = join(binDir, name);
  const script = `#!/bin/sh
echo "${name} failed" 1>&2
exit 7
`;
  await writeFile(scriptPath, script);
  await chmod(scriptPath, 0o755);
  return binDir;
}

async function waitForFile(path: string, timeoutMs = 2_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await access(path);
      return;
    } catch {
      await Bun.sleep(50);
    }
  }

  throw new Error(`Timed out waiting for file: ${path}`);
}

async function waitForTaskStatus(
  taskId: string,
  status: string,
  env: Record<string, string>,
  timeoutMs = 5_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await runCompiled(["task", "show", taskId, "--json"], tmpDir, { env });
    if (result.exitCode === 0) {
      const payload = expectJson<{ status?: string }>(result);
      if (payload.status === status) {
        return;
      }
    }
    await Bun.sleep(50);
  }

  throw new Error(`Timed out waiting for task ${taskId} to reach status ${status}`);
}

async function waitForConsumedHandoff(
  handoffId: string,
  env: Record<string, string>,
  timeoutMs = 5_000,
): Promise<{ status: string; consumedAt?: string; pickedUpByAgent?: string }> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await runCompiled(["handoff", "show", handoffId, "--json"], tmpDir, { env });
    if (result.exitCode === 0) {
      const payload = expectJson<{ status: string; consumedAt?: string; pickedUpByAgent?: string }>(result);
      if (payload.consumedAt) {
        return payload;
      }
    }
    await Bun.sleep(50);
  }

  throw new Error(`Timed out waiting for handoff ${handoffId} to be consumed`);
}

async function createActiveTask(
  title: string,
  sessionId = "codex-session-a",
): Promise<string> {
  const created = await runCompiled(["task", "q", title], tmpDir);
  expect(created.exitCode).toBe(0);
  const id = created.stdout.trim();
  await runCompiled(["task", "claim", id, "--session", sessionId, "--json"], tmpDir);
  await runCompiled(
    ["task", "update", id, "--status", "in_progress", "--session", sessionId, "--json"],
    tmpDir,
  );
  return id;
}

async function attachLockedContract(
  taskId: string,
  opts?: {
    scope?: string;
    ownerSessionId?: string;
    criterionText?: string;
    criterionKind?: "manual" | "receipt-hint";
  },
): Promise<string> {
  const scope = opts?.scope ?? "README.md";
  const ownerSessionId = opts?.ownerSessionId ?? "codex-session-a";
  const criterionText = opts?.criterionText ?? "pickup preserves contract ownership";
  const criterionKind = opts?.criterionKind ?? "manual";
  const templatePath = join(tmpDir, `contract-${taskId}.yaml`);
  await writeFile(
    templatePath,
    [
      "intent: keep ownership aligned across handoff pickup",
      "scope:",
      "  filesExpected:",
      `    - ${scope}`,
      "  filesForbidden: []",
      "doneWhen:",
      `  - text: ${criterionText}`,
      `    kind: ${criterionKind}`,
      "",
    ].join("\n"),
  );

  const created = await runCompiled(
    ["task", "contract", "new", taskId, "--from", templatePath, "--session", ownerSessionId, "--json"],
    tmpDir,
  );
  expect(created.exitCode).toBe(0);
  const contract = expectJson<{ id: string }>(created);

  const locked = await runCompiled(
    ["task", "contract", "lock", contract.id, "--session", ownerSessionId, "--json"],
    tmpDir,
  );
  expect(locked.exitCode).toBe(0);
  await rm(templatePath, { force: true });
  return contract.id;
}

// TODO(v2/phase-4): re-enable or remove. v1 `task claim` is detached per ADR-0007 big-bang;
// v2 equivalents live in src/v2/runtime/task.command.ts.
describe.skip("compiled handoff launcher E2E", () => {
  it(
    "launches codex by default, writes prompt artifacts, and returns launch metadata",
    async () => {
      const taskId = await createActiveTask("Investigate the bundle export regression");
      const argsPath = join(tmpDir, "codex-args.txt");
      const cwdPath = join(tmpDir, "codex-cwd.txt");
      const binDir = await installFakeProvider("codex", argsPath, cwdPath);

      const result = await runCompiled(
        ["handoff", "Investigate the bundle export regression", "--task-id", taskId, "--json"],
        tmpDir,
        {
          env: {
            HOME: tmpDir,
            USERPROFILE: tmpDir,
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
            FAKE_PROVIDER_ARGS: argsPath,
            FAKE_PROVIDER_CWD: cwdPath,
          },
        },
      );

      expect(result.exitCode).toBe(0);
      const record = expectJson<{
        id: string;
        agent: string;
        model: string;
        status: string;
        promptPath: string;
        outputPath: string;
        command: string[];
        refs: { taskId?: string };
        pid?: number;
      }>(result);

      expect(record.agent).toBe("codex");
      expect(record.model).toBe("gpt-5.4");
      expect(record.status).toBe("launched");
      expect(record.refs.taskId).toBe(taskId);
      expect(record.pid).toBeDefined();

      await waitForFile(argsPath);
      await waitForFile(cwdPath);

      const prompt = await readFile(join(tmpDir, record.promptPath), "utf8");
      expect(prompt).toContain("## Handoff Startup");
      expect(prompt).toContain(`maestro handoff pickup --id ${record.id} --json`);
      expect(prompt).toContain("## Task");
      expect(prompt).toContain("## Acceptance Criteria");
      expect(prompt).toContain("Next action:");

      const launchMeta = await readFile(join(tmpDir, ".maestro", "handoff", record.id, "handoff.json"), "utf8");
      expect(launchMeta).toContain('"agent": "codex"');
      expect(launchMeta).toContain(`"taskId": "${taskId}"`);

      const loggedArgs = await readFile(argsPath, "utf8");
      expect(loggedArgs).toContain("exec");
      expect(loggedArgs).toContain("--cd");
      expect(loggedArgs).toContain("--full-auto");
      expect(loggedArgs).toContain("gpt-5.4");

      const loggedCwd = (await readFile(cwdPath, "utf8")).trim();
      expect(loggedCwd.endsWith(basename(tmpDir))).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "launches claude in a sibling worktree and waits for completion when requested",
    async () => {
      const taskId = await createActiveTask("Finish the worktree handoff flow");
      const argsPath = join(tmpDir, "claude-args.txt");
      const cwdPath = join(tmpDir, "claude-cwd.txt");
      const binDir = await installFakeProvider("claude", argsPath, cwdPath);

      const result = await runCompiled(
        [
          "handoff",
          "Finish the worktree handoff flow",
          "--task-id",
          taskId,
          "--agent",
          "claude",
          "--worktree",
          "finish-worktree",
          "--wait",
          "--json",
        ],
        tmpDir,
        {
          env: {
            HOME: tmpDir,
            USERPROFILE: tmpDir,
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
            FAKE_PROVIDER_ARGS: argsPath,
            FAKE_PROVIDER_CWD: cwdPath,
          },
        },
      );

      expect(result.exitCode).toBe(0);
      const record = expectJson<{
        id: string;
        agent: string;
        model: string;
        status: string;
        exitCode?: number;
        worktree?: {
          path: string;
          branch: string;
          baseBranch: string;
        };
        outputPath: string;
      }>(result);

      expect(record.agent).toBe("claude");
      expect(record.model).toBe("opus");
      expect(record.status).toBe("completed");
      expect(record.exitCode).toBe(0);
      expect(record.worktree).toMatchObject({
        branch: "claude/finish-worktree",
        baseBranch: "main",
      });
      expect(record.worktree?.path).toBeDefined();
      expect(record.worktree!.path.endsWith(`${basename(tmpDir)}-finish-worktree`)).toBe(true);

      const loggedArgs = await readFile(argsPath, "utf8");
      expect(loggedArgs).toContain("--print");
      expect(loggedArgs).toContain("--permission-mode");
      expect(loggedArgs).toContain("bypassPermissions");
      expect(loggedArgs).toContain("opus");

      const loggedCwd = (await readFile(cwdPath, "utf8")).trim();
      expect(loggedCwd).toBe(record.worktree!.path);

      const outputLog = await readFile(join(tmpDir, record.outputPath), "utf8");
      expect(outputLog).toContain("claude output");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "creates a sibling worktree from the repo root when launched from a nested cwd",
    async () => {
      const taskId = await createActiveTask("Create the nested cwd worktree");
      const argsPath = join(tmpDir, "claude-nested-args.txt");
      const cwdPath = join(tmpDir, "claude-nested-cwd.txt");
      const nestedDir = join(tmpDir, "nested", "deeper");
      await Bun.$`mkdir -p ${nestedDir}`.quiet();
      const binDir = await installFakeProvider("claude", argsPath, cwdPath);

      const result = await runCompiled(
        [
          "handoff",
          "Create the nested cwd worktree",
          "--task-id",
          taskId,
          "--agent",
          "claude",
          "--worktree",
          "nested-cwd",
          "--wait",
          "--json",
        ],
        nestedDir,
        {
          env: {
            HOME: tmpDir,
            USERPROFILE: tmpDir,
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
            FAKE_PROVIDER_ARGS: argsPath,
            FAKE_PROVIDER_CWD: cwdPath,
          },
        },
      );

      expect(result.exitCode).toBe(0);
      const record = expectJson<{
        worktree?: {
          path: string;
          branch: string;
          baseBranch: string;
        };
      }>(result);

      expect(record.worktree).toMatchObject({
        branch: "claude/nested-cwd",
        baseBranch: "main",
      });
      expect(record.worktree?.path).toBeDefined();
      expect(record.worktree!.path.endsWith(`${basename(tmpDir)}-nested-cwd`)).toBe(true);
      expect(record.worktree!.path.includes(`${basename(nestedDir)}-nested-cwd`)).toBe(false);

      const loggedCwd = (await readFile(cwdPath, "utf8")).trim();
      expect(loggedCwd).toBe(record.worktree!.path);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "fails the CLI in --wait mode when the agent exits non-zero",
    async () => {
      const taskId = await createActiveTask("Fail the waited handoff");
      const binDir = await installFailingProvider("claude");

      const result = await runCompiled(
        ["handoff", "Fail the waited handoff", "--task-id", taskId, "--agent", "claude", "--wait", "--json"],
        tmpDir,
        {
          env: {
            HOME: tmpDir,
            USERPROFILE: tmpDir,
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
          },
        },
      );

      expect(result.exitCode).not.toBe(0);
      const payload = expectJson<{ error: string; hints: string[] }>(result);
      expect(payload.error).toContain("claude handoff exited with code 7");
      expect(payload.hints.join(" ")).toContain("Launch record:");

      const handoffDir = join(tmpDir, ".maestro", "handoff");
      const [handoffId] = await Bun.$`ls ${handoffDir}`.text().then((text) => text.trim().split("\n").filter(Boolean));
      expect(handoffId).toBeDefined();
      const handoffRecord = JSON.parse(await readFile(join(handoffDir, handoffId!, "handoff.json"), "utf8")) as {
        status: string;
        exitCode?: number;
      };
      expect(handoffRecord.status).toBe("failed");
      expect(handoffRecord.exitCode).toBe(7);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "consumes a handoff packet once and updates the linked task owner on pickup",
    async () => {
      const taskId = await createActiveTask("Transfer this task", "codex-session-a");
      const contractId = await attachLockedContract(taskId);
      const argsPath = join(tmpDir, "codex-pickup-args.txt");
      const cwdPath = join(tmpDir, "codex-pickup-cwd.txt");
      const binDir = await installFakeProvider("codex", argsPath, cwdPath);

      const handoffEnv = {
        HOME: tmpDir,
        USERPROFILE: tmpDir,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
      };

      const launched = await runCompiled(
        ["handoff", "Transfer this task", "--task-id", taskId, "--json"],
        tmpDir,
        { env: handoffEnv },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string }>(launched);

      const picked = await runCompiled(
        ["handoff", "pickup", "--id", record.id, "--agent", "claude", "--session", "pickup-1", "--json"],
        tmpDir,
        { env: handoffEnv },
      );
      expect(picked.exitCode).toBe(0);
      const consumed = expectJson<{ pickedUpByAgent?: string; pickedUpBySessionId?: string; consumedAt?: string }>(picked);
      expect(consumed.pickedUpByAgent).toBe("claude");
      expect(consumed.pickedUpBySessionId).toBe("pickup-1");
      expect(consumed.consumedAt).toBeDefined();

      const task = expectJson<{ assignee?: string; status: string }>(
        await runCompiled(["task", "show", taskId, "--json"], tmpDir),
      );
      expect(task).toMatchObject({
        assignee: "claude-code-pickup-1",
        status: "in_progress",
      });
      const contract = expectJson<{ id: string; lockedBy?: string; status: string }>(
        await runCompiled(["task", "contract", "show", contractId, "--json"], tmpDir),
      );
      expect(contract).toMatchObject({
        id: contractId,
        status: "locked",
        lockedBy: "claude-code-pickup-1",
      });

      const secondPickup = await runCompiled(
        ["handoff", "pickup", "--id", record.id, "--agent", "codex", "--session", "pickup-2", "--json"],
        tmpDir,
        { env: handoffEnv },
      );
      expect(secondPickup.exitCode).not.toBe(0);
      expect(expectJson<{ error: string }>(secondPickup).error).toContain("already consumed");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "has the launched receiver auto-pick up the packet before completing the linked task",
    async () => {
      const taskId = await createActiveTask("Auto pickup from launched receiver", "codex-session-a");
      const argsPath = join(tmpDir, "codex-auto-pickup-args.txt");
      const cwdPath = join(tmpDir, "codex-auto-pickup-cwd.txt");
      const promptPath = join(tmpDir, "codex-auto-pickup-prompt.txt");
      const binDir = await installBehavioralProvider("codex", argsPath, cwdPath, promptPath);

      const env = {
        HOME: tmpDir,
        USERPROFILE: tmpDir,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
        FAKE_PROVIDER_PROMPT: promptPath,
        FAKE_PROVIDER_MODE: "auto-pickup-complete",
        FAKE_PROVIDER_TASK_ID: taskId,
        FAKE_MAESTRO_BIN: DIST_CLI,
        CODEX_THREAD_ID: "",
        CLAUDECODE: "",
      };

      const launched = await runCompiled(
        ["handoff", "Auto pickup from launched receiver", "--task-id", taskId, "--json"],
        tmpDir,
        { env },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string; refs: { taskId?: string } }>(launched);
      expect(record.refs.taskId).toBe(taskId);

      await waitForFile(promptPath);
      const prompt = await readFile(promptPath, "utf8");
      expect(prompt).toContain(`maestro handoff pickup --id ${record.id} --json`);

      await waitForTaskStatus(taskId, "completed", env);
      const consumed = await waitForConsumedHandoff(record.id, env);
      expect(consumed.status).toBe("consumed");
      expect(consumed.pickedUpByAgent).toBe("codex");

      const listedOpen = await runCompiled(["handoff", "list", "--open", "--json"], tmpDir, { env });
      expect(listedOpen.exitCode).toBe(0);
      expect(expectJson<Array<{ id: string }>>(listedOpen).map((entry) => entry.id)).not.toContain(record.id);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "keeps contract verdicts fulfilled when the launched receiver produces runtime noise",
    async () => {
      const taskId = await createActiveTask("Runtime-noise contract handoff", "codex-session-a");
      const contractId = await attachLockedContract(taskId, {
        criterionKind: "receipt-hint",
        criterionText: "manual",
      });
      const runtimeDir = await mkdtemp(join(tmpdir(), "maestro-handoff-runtime-noise-"));
      cleanupDirs.push(runtimeDir);
      // Use a fakeHome OUTSIDE tmpDir so the handoff packet lands outside the
      // project's git tree and cannot leak into contract verdicts.
      const fakeHome = await mkdtemp(join(tmpdir(), "maestro-handoff-noise-home-"));
      cleanupDirs.push(fakeHome);
      const argsPath = join(runtimeDir, "codex-runtime-noise-args.txt");
      const cwdPath = join(runtimeDir, "codex-runtime-noise-cwd.txt");
      const promptPath = join(runtimeDir, "codex-runtime-noise-prompt.txt");
      const binDir = await installBehavioralProvider("codex", argsPath, cwdPath, promptPath, {
        binRoot: runtimeDir,
      });

      const env = {
        HOME: fakeHome,
        USERPROFILE: fakeHome,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
        FAKE_PROVIDER_PROMPT: promptPath,
        FAKE_PROVIDER_MODE: "auto-pickup-complete",
        FAKE_PROVIDER_TASK_ID: taskId,
        FAKE_PROVIDER_RUNTIME_NOISE: "1",
        FAKE_PROVIDER_WORK_FILE: "README.md",
        FAKE_PROVIDER_WORK_CONTENT: "# temp\nruntime clean\n",
        FAKE_PROVIDER_VERIFIED_BY: "manual",
        FAKE_MAESTRO_BIN: DIST_CLI,
        CODEX_THREAD_ID: "",
        CLAUDECODE: "",
      };

      const launched = await runCompiled(
        ["handoff", "Runtime-noise contract handoff", "--task-id", taskId, "--json"],
        tmpDir,
        { env },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string }>(launched);

      await waitForTaskStatus(taskId, "completed", env);
      await waitForConsumedHandoff(record.id, env);

      const shown = expectJson<{
        status: string;
        verdict?: {
          fulfilled: boolean;
          actualFilesTouched: string[];
          outOfScopeFiles: string[];
        };
      }>(await runCompiled(["task", "contract", "show", contractId, "--json"], tmpDir, { env }));
      expect(shown.status).toBe("fulfilled");
      expect(shown.verdict?.fulfilled).toBe(true);
      expect(shown.verdict?.actualFilesTouched).toContain("README.md");
      expect(shown.verdict?.actualFilesTouched).not.toContain(".maestro/config.yaml");
      expect(shown.verdict?.actualFilesTouched).not.toContain(".codex/config.toml");
      expect(shown.verdict?.actualFilesTouched).not.toContain(".claude/scheduled_tasks.lock");
      // With HOME pointing outside the project, the handoff packet is stored
      // at ~/.maestro/handoff/ and cannot appear in the project's git diff.
      expect(shown.verdict?.actualFilesTouched?.some((f) => f.includes("handoff"))).toBe(false);
      expect(shown.verdict?.outOfScopeFiles).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "reconciles a detached packet that finished the linked task without ever picking it up",
    async () => {
      const taskId = await createActiveTask("Detached receiver without pickup", "codex-session-a");
      const argsPath = join(tmpDir, "codex-stale-args.txt");
      const cwdPath = join(tmpDir, "codex-stale-cwd.txt");
      const promptPath = join(tmpDir, "codex-stale-prompt.txt");
      const binDir = await installBehavioralProvider("codex", argsPath, cwdPath, promptPath);

      const env = {
        HOME: tmpDir,
        USERPROFILE: tmpDir,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
        FAKE_PROVIDER_PROMPT: promptPath,
        FAKE_PROVIDER_MODE: "complete-without-pickup",
        FAKE_PROVIDER_TASK_ID: taskId,
        FAKE_MAESTRO_BIN: DIST_CLI,
        CODEX_THREAD_ID: "",
        CLAUDECODE: "",
      };

      const launched = await runCompiled(
        ["handoff", "Detached receiver without pickup", "--task-id", taskId, "--json"],
        tmpDir,
        { env },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string }>(launched);

      await waitForTaskStatus(taskId, "completed", env);

      const shown = await runCompiled(["handoff", "show", record.id, "--json"], tmpDir, { env });
      expect(shown.exitCode).toBe(0);
      const shownPayload = expectJson<{ status: string; consumedAt?: string }>(shown);
      expect(shownPayload.status).toBe("completed");
      expect(shownPayload.consumedAt).toBeUndefined();

      const listedOpen = await runCompiled(["handoff", "list", "--open", "--json"], tmpDir, { env });
      expect(listedOpen.exitCode).toBe(0);
      expect(expectJson<Array<{ id: string }>>(listedOpen).map((entry) => entry.id)).not.toContain(record.id);

      const task = expectJson<{ openHandoffs?: string[] }>(
        await runCompiled(["task", "show", taskId, "--json"], tmpDir, { env }),
      );
      expect(task.openHandoffs ?? []).toEqual([]);

      const picked = await runCompiled(["handoff", "pickup", "--id", record.id, "--json"], tmpDir, { env });
      expect(picked.exitCode).not.toBe(0);
      expect(expectJson<{ error: string }>(picked).error).toContain(
        `Handoff ${record.id} is already finished because linked task ${taskId} is completed`,
      );
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "launches a task-less handoff on first try and round-trips through pickup without creating a task",
    async () => {
      const fakeHome = join(tmpDir, "fake-home");
      await Bun.$`mkdir -p ${fakeHome}`.quiet();
      const argsPath = join(tmpDir, "claude-taskless-args.txt");
      const cwdPath = join(tmpDir, "claude-taskless-cwd.txt");
      const binDir = await installFakeProvider("claude", argsPath, cwdPath);

      const env = {
        HOME: fakeHome,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
      };

      const launched = await runCompiled(
        [
          "handoff",
          "Explore the AGENTS.md template",
          "--agent",
          "claude",
          "--name",
          "taskless walkthrough",
          "--json",
        ],
        tmpDir,
        { env },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{
        id: string;
        agent: string;
        status: string;
        refs: { taskId?: string };
      }>(launched);
      expect(record.agent).toBe("claude");
      expect(record.status).toBe("launched");
      expect(record.refs.taskId).toBeUndefined();

      const launchMeta = await readFile(
        join(fakeHome, ".maestro", "handoff", record.id, "handoff.json"),
        "utf8",
      );
      expect(launchMeta).not.toContain('"taskId"');

      const tasksBeforePickup = expectJson<readonly unknown[]>(
        await runCompiled(["task", "list", "--json"], tmpDir),
      );
      expect(tasksBeforePickup).toEqual([]);

      const picked = await runCompiled(
        [
          "handoff",
          "pickup",
          "--id",
          record.id,
          "--agent",
          "claude",
          "--session",
          "pickup-taskless",
          "--json",
        ],
        tmpDir,
        { env },
      );
      expect(picked.exitCode).toBe(0);
      const consumed = expectJson<{ consumedAt?: string }>(picked);
      expect(consumed.consumedAt).toBeDefined();

      const tasksAfterPickup = expectJson<readonly unknown[]>(
        await runCompiled(["task", "list", "--json"], tmpDir),
      );
      expect(tasksAfterPickup).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "picks up a task-less handoff with --agent only (no --session required)",
    async () => {
      const fakeHome = join(tmpDir, "fake-home-agentonly");
      await Bun.$`mkdir -p ${fakeHome}`.quiet();
      const argsPath = join(tmpDir, "claude-pickup-agentonly-args.txt");
      const cwdPath = join(tmpDir, "claude-pickup-agentonly-cwd.txt");
      const binDir = await installFakeProvider("claude", argsPath, cwdPath);

      const env = {
        HOME: fakeHome,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
      };

      const launched = await runCompiled(
        ["handoff", "pickup smoke", "--agent", "claude", "--name", "pickup smoke", "--json"],
        tmpDir,
        { env },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string; refs: { taskId?: string } }>(launched);
      expect(record.refs.taskId).toBeUndefined();

      const picked = await runCompiled(
        ["handoff", "pickup", "--id", record.id, "--agent", "claude", "--json"],
        tmpDir,
        { env },
      );
      expect(picked.exitCode).toBe(0);
      const consumed = expectJson<{
        pickedUpByAgent?: string;
        pickedUpBySessionId?: string;
        consumedAt?: string;
      }>(picked);
      expect(consumed.pickedUpByAgent).toBe("claude");
      expect(consumed.pickedUpBySessionId).toBeUndefined();
      expect(consumed.consumedAt).toBeDefined();
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "picks up a task-less handoff created in a different workspace via the global store",
    async () => {
      const fakeHome = await mkdtemp(join(tmpdir(), "maestro-handoff-home-"));
      const workspaceA = await mkdtemp(join(tmpdir(), "maestro-handoff-ws-a-"));
      const workspaceB = await mkdtemp(join(tmpdir(), "maestro-handoff-ws-b-"));
      try {
        await initGitRepo(workspaceA);
        await runCommand(["git", "config", "user.name", "Test User"], workspaceA);
        await runCommand(["git", "config", "user.email", "test@example.com"], workspaceA);
        await writeFile(join(workspaceA, "README.md"), "# A\n");
        await runCommand(["git", "add", "README.md"], workspaceA);
        await runCommand(["git", "commit", "-m", "init"], workspaceA);

        await initGitRepo(workspaceB);
        await runCommand(["git", "config", "user.name", "Test User"], workspaceB);
        await runCommand(["git", "config", "user.email", "test@example.com"], workspaceB);
        await writeFile(join(workspaceB, "README.md"), "# B\n");
        await runCommand(["git", "add", "README.md"], workspaceB);
        await runCommand(["git", "commit", "-m", "init"], workspaceB);

        const argsPath = join(fakeHome, "claude-cross-args.txt");
        const cwdPath = join(fakeHome, "claude-cross-cwd.txt");
        const binDir = join(fakeHome, "bin");
        await Bun.$`mkdir -p ${binDir}`.quiet();
        const fakeClaude = join(binDir, "claude");
        await writeFile(
          fakeClaude,
          `#!/bin/sh\nprintf '%s\\n' "$PWD" > "$FAKE_PROVIDER_CWD"\nprintf '%s\\n' "$@" > "$FAKE_PROVIDER_ARGS"\necho claude output\n`,
        );
        await chmod(fakeClaude, 0o755);

        const commonEnv = {
          HOME: fakeHome,
          PATH: `${binDir}:${process.env.PATH ?? ""}`,
          FAKE_PROVIDER_ARGS: argsPath,
          FAKE_PROVIDER_CWD: cwdPath,
        };

        const launched = await runCompiled(
          ["handoff", "cross-workspace task", "--agent", "claude", "--json"],
          workspaceA,
          { env: commonEnv },
        );
        expect(launched.exitCode).toBe(0);
        const record = expectJson<{ id: string; refs: { taskId?: string } }>(launched);
        expect(record.refs.taskId).toBeUndefined();

        await access(join(fakeHome, ".maestro", "handoff", record.id, "handoff.json"));
        await expect(
          access(join(workspaceA, ".maestro", "handoff", record.id, "handoff.json")),
        ).rejects.toThrow();

        const picked = await runCompiled(
          ["handoff", "pickup", "--id", record.id, "--agent", "claude", "--json"],
          workspaceB,
          { env: commonEnv },
        );
        expect(picked.exitCode).toBe(0);
        const consumed = expectJson<{ consumedAt?: string; pickedUpByAgent?: string }>(picked);
        expect(consumed.consumedAt).toBeDefined();
        expect(consumed.pickedUpByAgent).toBe("claude");
      } finally {
        await rm(fakeHome, { recursive: true, force: true });
        await rm(workspaceA, { recursive: true, force: true });
        await rm(workspaceB, { recursive: true, force: true });
      }
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "enumerates open packet ids when pickup is ambiguous",
    async () => {
      const fakeHome = join(tmpDir, "fake-home-ambiguous");
      await Bun.$`mkdir -p ${fakeHome}`.quiet();
      const argsPath = join(tmpDir, "claude-ambig-args.txt");
      const cwdPath = join(tmpDir, "claude-ambig-cwd.txt");
      const binDir = await installFakeProvider("claude", argsPath, cwdPath);

      const env = {
        HOME: fakeHome,
        PATH: `${binDir}:${process.env.PATH ?? ""}`,
        FAKE_PROVIDER_ARGS: argsPath,
        FAKE_PROVIDER_CWD: cwdPath,
      };

      const first = await runCompiled(
        ["handoff", "first ambiguous task", "--agent", "claude", "--json"],
        tmpDir,
        { env },
      );
      expect(first.exitCode).toBe(0);
      const firstRecord = expectJson<{ id: string }>(first);

      const second = await runCompiled(
        ["handoff", "second ambiguous task", "--agent", "claude", "--json"],
        tmpDir,
        { env },
      );
      expect(second.exitCode).toBe(0);
      const secondRecord = expectJson<{ id: string }>(second);

      const picked = await runCompiled(
        ["handoff", "pickup", "--agent", "claude", "--json"],
        tmpDir,
        { env },
      );
      expect(picked.exitCode).not.toBe(0);
      const err = expectJson<{ error: string; hints: string[] }>(picked);
      expect(err.error).toContain("ambiguous");
      const hintsBlob = err.hints.join("\n");
      expect(hintsBlob).toContain(firstRecord.id);
      expect(hintsBlob).toContain(secondRecord.id);
      expect(hintsBlob).toContain("first ambiguous task");
      expect(hintsBlob).toContain("second ambiguous task");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
