import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsLaunchStoreAdapter } from "@/features/handoff";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

const noDetectedSessionEnv = {
  CODEX_THREAD_ID: "",
  CLAUDECODE: "",
};

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-handoff-"));
  await initGitRepo(tmpDir);
  const init = await runCli(["init"], tmpDir, { env: noDetectedSessionEnv });
  expect(init.exitCode).toBe(0);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task + handoff pickup CLI", () => {
  it("keeps standalone pickup with --agent only valid when no session is detected", async () => {
    const launchStore = new FsLaunchStoreAdapter(tmpDir);
    const launch = await launchStore.create({
      task: "prompt only",
      name: "prompt only",
      agent: "claude",
      model: "opus",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: {},
      prompt: "## Task\n\nprompt only\n",
    });

    const picked = await runCli(
      ["handoff", "pickup", "--id", launch.id, "--agent", "codex", "--json"],
      tmpDir,
      { env: noDetectedSessionEnv },
    );
    expect(picked.exitCode).toBe(0);
    const payload = expectJson<{
      pickedUpByAgent?: string;
      pickedUpBySessionId?: string;
      consumedAt?: string;
    }>(picked);
    expect(payload.pickedUpByAgent).toBe("codex");
    expect(payload.pickedUpBySessionId).toBeUndefined();
    expect(payload.consumedAt).toBeDefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("keeps task ownership compatible with subsequent task updates after pickup without a detected session", async () => {
    const created = await runCli(["task", "create", "handoff pickup continuity", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(started.exitCode).toBe(0);

    const beforePickup = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    const before = expectJson<{ assignee?: string }>(beforePickup);
    expect(before.assignee).toBeTruthy();

    const launchStore = new FsLaunchStoreAdapter(tmpDir);
    const launch = await launchStore.create({
      task: "task linked",
      name: "task linked",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: task.id },
      prompt: "## Task\n\ntask linked\n",
    });

    const picked = await runCli(["handoff", "pickup", "--id", launch.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(picked.exitCode).toBe(0);

    const afterPickup = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    const resumed = expectJson<{ assignee?: string; status: string }>(afterPickup);
    expect(resumed.status).toBe("in_progress");
    expect(resumed.assignee).toBe(before.assignee);

    const updated = await runCli(["task", "update", task.id, "--current-state", "after pickup", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(updated.exitCode).toBe(0);
    expect(expectJson<{ id: string }>(updated).id).toBe(task.id);
  }, SLOW_CLI_TIMEOUT_MS);

  it("reconciles stale launched packets after the linked task completes and hides them from open handoff surfaces", async () => {
    const created = await runCli(["task", "create", "stale handoff completion", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(started.exitCode).toBe(0);

    const launchStore = new FsLaunchStoreAdapter(tmpDir);
    const launch = await launchStore.create({
      task: "task linked stale packet",
      name: "task linked stale packet",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: task.id },
      prompt: "## Task\n\ntask linked stale packet\n",
    });

    const completed = await runCli(
      ["task", "update", task.id, "--status", "completed", "--reason", "done", "--json"],
      tmpDir,
      { env: noDetectedSessionEnv },
    );
    expect(completed.exitCode).toBe(0);

    const shown = await runCli(["handoff", "show", launch.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(shown.exitCode).toBe(0);
    const shownPayload = expectJson<{ id: string; status: string; consumedAt?: string }>(shown);
    expect(shownPayload.id).toBe(launch.id);
    expect(shownPayload.status).toBe("completed");
    expect(shownPayload.consumedAt).toBeUndefined();

    const listedOpen = await runCli(["handoff", "list", "--open", "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(listedOpen.exitCode).toBe(0);
    expect(expectJson<Array<{ id: string }>>(listedOpen).map((record) => record.id)).not.toContain(launch.id);

    const taskView = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(taskView.exitCode).toBe(0);
    expect(expectJson<{ openHandoffs?: string[] }>(taskView).openHandoffs ?? []).toEqual([]);

    const picked = await runCli(["handoff", "pickup", "--id", launch.id, "--json"], tmpDir, {
      env: noDetectedSessionEnv,
    });
    expect(picked.exitCode).not.toBe(0);
    expect(expectJson<{ error: string }>(picked).error).toContain(
      `Handoff ${launch.id} is already finished because linked task ${task.id} is completed`,
    );
  }, SLOW_CLI_TIMEOUT_MS);
});
