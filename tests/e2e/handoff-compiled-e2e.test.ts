import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { access, chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { tmpdir } from "node:os";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-handoff-e2e-"));
  await initGitRepo(tmpDir);
  await runCommand(["git", "config", "user.name", "Test User"], tmpDir);
  await runCommand(["git", "config", "user.email", "test@example.com"], tmpDir);
  await writeFile(join(tmpDir, "README.md"), "# temp\n");
  await runCommand(["git", "add", "README.md"], tmpDir);
  await runCommand(["git", "commit", "-m", "init"], tmpDir);
});

afterEach(async () => {
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
  scope = "README.md",
  ownerSessionId = "codex-session-a",
): Promise<string> {
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
      "  - text: pickup preserves contract ownership",
      "    kind: manual",
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
  return contract.id;
}

describe.skipIf(process.platform === "win32")("compiled handoff launcher E2E", () => {
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
      expect(prompt).toContain("## Task");
      expect(prompt).toContain("## Acceptance Criteria");
      expect(prompt).toContain("Next action:");

      const launchMeta = await readFile(join(tmpDir, ".maestro", "launches", record.id, "launch.json"), "utf8");
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
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
          },
        },
      );

      expect(result.exitCode).not.toBe(0);
      const payload = expectJson<{ error: string; hints: string[] }>(result);
      expect(payload.error).toContain("claude handoff exited with code 7");
      expect(payload.hints.join(" ")).toContain("Launch record:");

      const launchesDir = join(tmpDir, ".maestro", "launches");
      const [launchId] = await Bun.$`ls ${launchesDir}`.text().then((text) => text.trim().split("\n").filter(Boolean));
      expect(launchId).toBeDefined();
      const launchRecord = JSON.parse(await readFile(join(launchesDir, launchId!, "launch.json"), "utf8")) as {
        status: string;
        exitCode?: number;
      };
      expect(launchRecord.status).toBe("failed");
      expect(launchRecord.exitCode).toBe(7);
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

      const launched = await runCompiled(
        ["handoff", "Transfer this task", "--task-id", taskId, "--json"],
        tmpDir,
        {
          env: {
            PATH: `${binDir}:${process.env.PATH ?? ""}`,
            FAKE_PROVIDER_ARGS: argsPath,
            FAKE_PROVIDER_CWD: cwdPath,
          },
        },
      );
      expect(launched.exitCode).toBe(0);
      const record = expectJson<{ id: string }>(launched);

      const picked = await runCompiled(
        ["handoff", "pickup", "--id", record.id, "--agent", "claude", "--session", "pickup-1", "--json"],
        tmpDir,
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
      );
      expect(secondPickup.exitCode).not.toBe(0);
      expect(expectJson<{ error: string }>(secondPickup).error).toContain("already consumed");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
