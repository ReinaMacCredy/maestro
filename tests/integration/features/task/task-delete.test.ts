import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-delete-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task delete", () => {
  it("requires the owner session or force before deleting a claimed task", async () => {
    const created = await runCli(["task", "create", "claimed delete", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);

    await runCli(["task", "claim", task.id, "--session", "delete-owner", "--json"], tmpDir);

    const missingSession = await runCli(["task", "delete", task.id], tmpDir);
    expect(missingSession.exitCode).not.toBe(0);
    expect(missingSession.stderr).toContain(`Task ${task.id} is claimed by delete-owner`);
    expect(missingSession.stderr).toContain("delete");

    const wrongSession = await runCli(["task", "delete", task.id, "--session", "delete-other"], tmpDir);
    expect(wrongSession.exitCode).not.toBe(0);
    expect(wrongSession.stderr).toContain("current session cannot 'delete' it");

    const deleted = await runCli(["task", "delete", task.id, "--session", "delete-owner", "--json"], tmpDir);
    expect(expectJson<{ id: string }>(deleted).id).toBe(task.id);

    const shown = await runCli(["task", "show", task.id], tmpDir);
    expect(shown.exitCode).not.toBe(0);
    expect(shown.stderr).toContain(`Task ${task.id} not found`);
  }, SLOW_CLI_TIMEOUT_MS);

  it("does not traverse out of task state when asked to delete an invalid id", async () => {
    const continuationPath = join(tmpDir, ".maestro", "tasks", "continuations", "keep.json");
    const historyPath = join(tmpDir, ".maestro", "tasks", "keep.jsonl");
    await mkdir(join(tmpDir, ".maestro", "tasks", "continuations"), { recursive: true });
    await mkdir(join(tmpDir, ".maestro", "tasks"), { recursive: true });
    await Bun.write(continuationPath, "{\"sentinel\":true}\n");
    await Bun.write(historyPath, "{\"sentinel\":true}\n");

    const invalid = await runCli(["task", "delete", "../keep"], tmpDir);
    expect(invalid.exitCode).not.toBe(0);
    expect(invalid.stderr).toContain("Task ../keep not found");

    expect(await Bun.file(continuationPath).exists()).toBe(true);
    expect(await Bun.file(historyPath).exists()).toBe(true);
  }, SLOW_CLI_TIMEOUT_MS);
});
