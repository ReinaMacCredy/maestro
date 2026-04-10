import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "../../../helpers/run-cli.js";
import { expectJson } from "../../../helpers/run-compiled-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

async function initGitRepo(cwd: string): Promise<void> {
  const init = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await init.exited;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task CLI daily loop", () => {
  it("completes the full create -> ready -> close -> ready cycle", async () => {
    const captured = await runCli(
      ["task", "q", "login endpoint", "--priority", "1"],
      tmpDir,
    );
    expect(captured.exitCode).toBe(0);
    const apiId = captured.stdout;
    expect(apiId).toMatch(/^tsk-[0-9a-f]{6}$/);

    const dependent = await runCli(
      [
        "task",
        "create",
        "JWT middleware",
        "--depends-on",
        apiId,
        "--priority",
        "1",
        "--json",
      ],
      tmpDir,
    );
    expect(dependent.exitCode).toBe(0);
    const mw = expectJson<{ id: string; dependsOn: string[] }>(dependent);
    expect(mw.id).toMatch(/^tsk-[0-9a-f]{6}$/);
    expect(mw.dependsOn).toEqual([apiId]);

    const readyBefore = await runCli(["task", "ready", "--json"], tmpDir);
    const beforeList = expectJson<Array<{ id: string; title: string }>>(
      readyBefore,
    );
    expect(beforeList.length).toBe(1);
    expect(beforeList[0]?.id).toBe(apiId);

    const closed = await runCli(
      ["task", "close", apiId, "--reason", "shipped"],
      tmpDir,
    );
    expect(closed.exitCode).toBe(0);
    expect(closed.stdout).toContain("Task closed:");
    expect(closed.stdout).toContain("Reason: shipped");

    const readyAfter = await runCli(["task", "ready", "--json"], tmpDir);
    const afterList = expectJson<Array<{ id: string; title: string }>>(
      readyAfter,
    );
    expect(afterList.length).toBe(1);
    expect(afterList[0]?.id).toBe(mw.id);

    const showClosed = await runCli(["task", "show", apiId, "--json"], tmpDir);
    const closedTask = expectJson<{
      id: string;
      status: string;
      closeReason: string;
    }>(showClosed);
    expect(closedTask.status).toBe("closed");
    expect(closedTask.closeReason).toBe("shipped");

    const listOpen = await runCli(
      ["task", "list", "--status", "open", "--json"],
      tmpDir,
    );
    const openList = expectJson<Array<{ id: string; status: string }>>(listOpen);
    expect(openList.length).toBe(1);
    expect(openList[0]?.id).toBe(mw.id);
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects --status closed via update (must use close)", async () => {
    const create = await runCli(["task", "q", "direct close attempt"], tmpDir);
    expect(create.exitCode).toBe(0);
    const id = create.stdout;

    const bad = await runCli(
      ["task", "update", id, "--status", "closed"],
      tmpDir,
    );
    expect(bad.exitCode).not.toBe(0);
    expect(bad.stderr).toContain("Cannot set status to 'closed'");
  }, SLOW_CLI_TIMEOUT_MS);

  it("update --add-label and --remove-label", async () => {
    const created = await runCli(
      ["task", "create", "labeled", "--labels", "auth,ui", "--json"],
      tmpDir,
    );
    const id = expectJson<{ id: string }>(created).id;

    const added = await runCli(
      ["task", "update", id, "--add-label", "urgent", "--json"],
      tmpDir,
    );
    expect(expectJson<{ labels: string[] }>(added).labels).toEqual([
      "auth",
      "ui",
      "urgent",
    ]);

    const removed = await runCli(
      ["task", "update", id, "--remove-label", "auth", "--json"],
      tmpDir,
    );
    expect(expectJson<{ labels: string[] }>(removed).labels).toEqual([
      "ui",
      "urgent",
    ]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("hides deferred tasks from ready until explicitly included", async () => {
    const created = await runCli(
      ["task", "create", "defer me", "--json"],
      tmpDir,
    );
    const id = expectJson<{ id: string }>(created).id;

    const deferred = await runCli(
      ["task", "update", id, "--status", "deferred"],
      tmpDir,
    );
    expect(deferred.exitCode).toBe(0);

    const hidden = await runCli(["task", "ready", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(hidden)).toEqual([]);

    const included = await runCli(
      ["task", "ready", "--json", "--include-deferred"],
      tmpDir,
    );
    expect(expectJson<Array<{ id: string }>>(included).map((task) => task.id)).toEqual([id]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("sanitizes terminal escape sequences in task text output", async () => {
    const create = await runCli(
      ["task", "create", "\u001b[31mred title\u001b[0m"],
      tmpDir,
    );
    expect(create.exitCode).toBe(0);
    expect(create.stdout).not.toContain("\u001b");
    expect(create.stdout).toContain("red title");

    const listed = await runCli(["task", "list"], tmpDir);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).not.toContain("\u001b");
    expect(listed.stdout).toContain("red title");
  }, SLOW_CLI_TIMEOUT_MS);
});
