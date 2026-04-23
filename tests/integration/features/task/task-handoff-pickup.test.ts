import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsHandoffStoreAdapter } from "@/features/handoff";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

// The production handoff store writes to ~/.maestro/handoff/. Redirect HOME to
// the tmp dir so the CLI and the test-side adapter share the same root.
const baseEnv = (tmp: string): Record<string, string> => ({
  CODEX_THREAD_ID: "",
  CLAUDECODE: "",
  HOME: tmp,
  USERPROFILE: tmp,
});

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-handoff-"));
  await initGitRepo(tmpDir);
  const init = await runCli(["init"], tmpDir, { env: baseEnv(tmpDir) });
  expect(init.exitCode).toBe(0);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task + handoff pickup CLI", () => {
  it("keeps standalone pickup with --agent only valid when no session is detected", async () => {
    const handoffStore = new FsHandoffStoreAdapter(tmpDir);
    const handoff = await handoffStore.create({
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
      ["handoff", "pickup", "--id", handoff.id, "--agent", "codex", "--json"],
      tmpDir,
      { env: baseEnv(tmpDir) },
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
      env: baseEnv(tmpDir),
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(started.exitCode).toBe(0);

    const beforePickup = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    const before = expectJson<{ assignee?: string }>(beforePickup);
    expect(before.assignee).toBeTruthy();

    const handoffStore = new FsHandoffStoreAdapter(tmpDir);
    const handoff = await handoffStore.create({
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

    const picked = await runCli(["handoff", "pickup", "--id", handoff.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(picked.exitCode).toBe(0);

    const afterPickup = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    const resumed = expectJson<{ assignee?: string; status: string }>(afterPickup);
    expect(resumed.status).toBe("in_progress");
    expect(resumed.assignee).toBe(before.assignee);

    const updated = await runCli(["task", "update", task.id, "--current-state", "after pickup", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(updated.exitCode).toBe(0);
    expect(expectJson<{ id: string }>(updated).id).toBe(task.id);
  }, SLOW_CLI_TIMEOUT_MS);

  it("reconciles stale launched packets after the linked task completes and hides them from open handoff surfaces", async () => {
    const created = await runCli(["task", "create", "stale handoff completion", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(started.exitCode).toBe(0);

    const handoffStore = new FsHandoffStoreAdapter(tmpDir);
    const handoff = await handoffStore.create({
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
      { env: baseEnv(tmpDir) },
    );
    expect(completed.exitCode).toBe(0);

    const shown = await runCli(["handoff", "show", handoff.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(shown.exitCode).toBe(0);
    const shownPayload = expectJson<{ id: string; status: string; consumedAt?: string }>(shown);
    expect(shownPayload.id).toBe(handoff.id);
    expect(shownPayload.status).toBe("completed");
    expect(shownPayload.consumedAt).toBeUndefined();

    const listedOpen = await runCli(["handoff", "list", "--open", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(listedOpen.exitCode).toBe(0);
    expect(expectJson<Array<{ id: string }>>(listedOpen).map((record) => record.id)).not.toContain(handoff.id);

    const taskView = await runCli(["task", "show", task.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(taskView.exitCode).toBe(0);
    expect(expectJson<{ openHandoffs?: string[] }>(taskView).openHandoffs ?? []).toEqual([]);

    const picked = await runCli(["handoff", "pickup", "--id", handoff.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(picked.exitCode).not.toBe(0);
    expect(expectJson<{ error: string }>(picked).error).toContain(
      `Handoff ${handoff.id} is already finished because linked task ${task.id} is completed`,
    );
  }, SLOW_CLI_TIMEOUT_MS);

  it("keeps unrelated task show calls from reconciling another task's stale handoff", async () => {
    const createdA = await runCli(["task", "create", "unrelated task", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(createdA.exitCode).toBe(0);
    const taskA = expectJson<{ id: string }>(createdA);

    const createdB = await runCli(["task", "create", "handoff owner task", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(createdB.exitCode).toBe(0);
    const taskB = expectJson<{ id: string }>(createdB);

    const startedB = await runCli(["task", "update", taskB.id, "--status", "in_progress", "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(startedB.exitCode).toBe(0);

    const handoffStore = new FsHandoffStoreAdapter(tmpDir);
    const handoff = await handoffStore.create({
      task: "linked to task B",
      name: "linked to task B",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: tmpDir,
      targetDir: tmpDir,
      refs: { taskId: taskB.id },
      prompt: "## Task\n\nlinked to task B\n",
    });
    await handoffStore.update({ ...handoff, status: "launched" });

    const completedB = await runCli(
      ["task", "update", taskB.id, "--status", "completed", "--reason", "done", "--json"],
      tmpDir,
      { env: baseEnv(tmpDir) },
    );
    expect(completedB.exitCode).toBe(0);
    expect((await handoffStore.get(handoff.id))?.status).toBe("launched");

    const shownA = await runCli(["task", "show", taskA.id, "--json"], tmpDir, {
      env: baseEnv(tmpDir),
    });
    expect(shownA.exitCode).toBe(0);
    expect((await handoffStore.get(handoff.id))?.status).toBe("launched");
  }, SLOW_CLI_TIMEOUT_MS);

  it("does not reconcile a foreign project's stale handoff when task ids collide in the global store", async () => {
    const sharedHome = join(tmpDir, "shared-home");
    const projectA = join(tmpDir, "project-a");
    const projectB = join(tmpDir, "project-b");

    await mkdir(sharedHome, { recursive: true });
    await mkdir(projectA, { recursive: true });
    await mkdir(projectB, { recursive: true });
    await initGitRepo(projectA);
    await initGitRepo(projectB);
    expect((await runCli(["init"], projectA, { env: baseEnv(sharedHome) })).exitCode).toBe(0);
    expect((await runCli(["init"], projectB, { env: baseEnv(sharedHome) })).exitCode).toBe(0);

    const created = await runCli(["task", "create", "project-b task", "--json"], projectB, {
      env: baseEnv(sharedHome),
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], projectB, {
      env: baseEnv(sharedHome),
    });
    expect(started.exitCode).toBe(0);

    const handoffStore = new FsHandoffStoreAdapter(sharedHome);
    const handoff = await handoffStore.create({
      task: "foreign task-id collision",
      name: "foreign task-id collision",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: projectA,
      targetDir: projectA,
      refs: { taskId: task.id },
      prompt: "## Task\n\nforeign task-id collision\n",
    });
    await handoffStore.update({ ...handoff, status: "launched" });

    const completed = await runCli(
      ["task", "update", task.id, "--status", "completed", "--reason", "done", "--json"],
      projectB,
      { env: baseEnv(sharedHome) },
    );
    expect(completed.exitCode).toBe(0);
    expect((await handoffStore.get(handoff.id))?.status).toBe("launched");

    const shown = await runCli(["task", "show", task.id, "--json"], projectB, {
      env: baseEnv(sharedHome),
    });
    expect(shown.exitCode).toBe(0);
    expect(expectJson<{ openHandoffs?: string[] }>(shown).openHandoffs ?? []).toEqual([]);
    expect((await handoffStore.get(handoff.id))?.status).toBe("launched");
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects foreign-project task-linked pickup unless --standalone is passed", async () => {
    const sharedHome = join(tmpDir, "shared-home");
    const projectA = join(tmpDir, "project-a");
    const projectB = join(tmpDir, "project-b");

    await mkdir(sharedHome, { recursive: true });
    await mkdir(projectA, { recursive: true });
    await mkdir(projectB, { recursive: true });
    await initGitRepo(projectA);
    await initGitRepo(projectB);
    expect((await runCli(["init"], projectA, { env: baseEnv(sharedHome) })).exitCode).toBe(0);
    expect((await runCli(["init"], projectB, { env: baseEnv(sharedHome) })).exitCode).toBe(0);

    const created = await runCli(["task", "create", "project-a handoff task", "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(started.exitCode).toBe(0);

    const handoffStore = new FsHandoffStoreAdapter(sharedHome);
    const sourceProjectRoot = resolveMaestroProjectRoot(projectA);
    const handoff = await handoffStore.create({
      task: "project-a linked handoff",
      name: "project-a linked handoff",
      agent: "codex",
      model: "gpt-5.4",
      wait: false,
      sourceDir: projectA,
      targetDir: projectA,
      refs: { taskId: task.id },
      prompt: "## Task\n\nproject-a linked handoff\n",
    });

    const picked = await runCli(["handoff", "pickup", "--id", handoff.id, "--json"], projectB, {
      env: baseEnv(sharedHome),
    });
    expect(picked.exitCode).not.toBe(0);
    const payload = expectJson<{ error: string; hints?: string[] }>(picked);
    expect(payload.error).toContain(`Handoff ${handoff.id} belongs to project ${sourceProjectRoot}`);
    expect(payload.hints ?? []).toContain(
      `Pick it up from the source project to preserve task ownership: cd ${JSON.stringify(sourceProjectRoot)} && maestro handoff pickup --id ${handoff.id} --json`,
    );

    const reloaded = await handoffStore.get(handoff.id);
    expect(reloaded?.consumedAt).toBeUndefined();

    const sourceTask = await runCli(["task", "show", task.id, "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(sourceTask.exitCode).toBe(0);
    expect(expectJson<{ status: string }>(sourceTask).status).toBe("in_progress");
  }, SLOW_CLI_TIMEOUT_MS);

  it("allows explicit standalone pickup for a foreign task-linked packet", async () => {
    const sharedHome = join(tmpDir, "shared-home");
    const projectA = join(tmpDir, "project-a");
    const projectB = join(tmpDir, "project-b");

    await mkdir(sharedHome, { recursive: true });
    await mkdir(projectA, { recursive: true });
    await mkdir(projectB, { recursive: true });
    await initGitRepo(projectA);
    await initGitRepo(projectB);
    expect((await runCli(["init"], projectA, { env: baseEnv(sharedHome) })).exitCode).toBe(0);
    expect((await runCli(["init"], projectB, { env: baseEnv(sharedHome) })).exitCode).toBe(0);

    const created = await runCli(["task", "create", "project-a standalone override", "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(created.exitCode).toBe(0);
    const task = expectJson<{ id: string }>(created);

    const started = await runCli(["task", "update", task.id, "--status", "in_progress", "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(started.exitCode).toBe(0);
    const before = expectJson<{ assignee?: string }>(await runCli(["task", "show", task.id, "--json"], projectA, {
      env: baseEnv(sharedHome),
    }));

    const handoffStore = new FsHandoffStoreAdapter(sharedHome);
    const handoff = await handoffStore.create({
      task: "standalone override",
      name: "standalone override",
      agent: "claude",
      model: "opus",
      wait: false,
      sourceDir: projectA,
      targetDir: projectA,
      refs: { taskId: task.id },
      prompt: "## Task\n\nstandalone override\n",
    });

    const picked = await runCli(
      ["handoff", "pickup", "--id", handoff.id, "--standalone", "--json"],
      projectB,
      { env: baseEnv(sharedHome) },
    );
    expect(picked.exitCode).toBe(0);
    const payload = expectJson<{
      pickedUpByAgent?: string;
      pickedUpBySessionId?: string;
      consumedAt?: string;
    }>(picked);
    expect(payload.consumedAt).toBeDefined();
    expect(payload.pickedUpByAgent).toBe("claude");
    expect(picked.stderr).toContain(`Linked task ${task.id} was left unchanged`);

    const sourceTask = await runCli(["task", "show", task.id, "--json"], projectA, {
      env: baseEnv(sharedHome),
    });
    expect(sourceTask.exitCode).toBe(0);
    const after = expectJson<{ assignee?: string; status: string }>(sourceTask);
    expect(after.status).toBe("in_progress");
    expect(after.assignee).toBe(before.assignee);
  }, SLOW_CLI_TIMEOUT_MS);

  it("allows prompt-only pickup from a different project", async () => {
    const sharedHome = join(tmpDir, "shared-home");
    const projectA = join(tmpDir, "project-a");
    const projectB = join(tmpDir, "project-b");

    await mkdir(sharedHome, { recursive: true });
    await mkdir(projectA, { recursive: true });
    await mkdir(projectB, { recursive: true });
    await initGitRepo(projectA);
    await initGitRepo(projectB);
    expect((await runCli(["init"], projectA, { env: baseEnv(sharedHome) })).exitCode).toBe(0);
    expect((await runCli(["init"], projectB, { env: baseEnv(sharedHome) })).exitCode).toBe(0);

    const handoffStore = new FsHandoffStoreAdapter(sharedHome);
    const handoff = await handoffStore.create({
      task: "prompt-only cross project",
      name: "prompt-only cross project",
      agent: "claude",
      model: "opus",
      wait: false,
      sourceDir: projectA,
      targetDir: projectA,
      refs: {},
      prompt: "## Task\n\nprompt-only cross project\n",
    });

    const picked = await runCli(["handoff", "pickup", "--id", handoff.id, "--json"], projectB, {
      env: baseEnv(sharedHome),
    });
    expect(picked.exitCode).toBe(0);
    const payload = expectJson<{ consumedAt?: string; pickedUpByAgent?: string }>(picked);
    expect(payload.consumedAt).toBeDefined();
    expect(payload.pickedUpByAgent).toBe("claude");
  }, SLOW_CLI_TIMEOUT_MS);
});
