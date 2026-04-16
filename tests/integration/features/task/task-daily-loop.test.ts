import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expectJson, initGitRepo } from "../../../helpers/run-compiled-cli.js";
import { runCli } from "../../../helpers/run-cli.js";

const SLOW_CLI_TIMEOUT_MS = 30_000;

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-task-cli-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("task CLI daily loop", () => {
  it("completes the create -> claim -> start -> complete -> ready cycle", async () => {
    const captured = await runCli(["task", "q", "login endpoint", "--priority", "1"], tmpDir);
    const apiId = captured.stdout;

    const dependent = await runCli(
      ["task", "create", "JWT middleware", "--blocked-by", apiId, "--priority", "1", "--json"],
      tmpDir,
    );
    const mw = expectJson<{ id: string; blockedBy: string[]; status: string }>(dependent);
    expect(mw.status).toBe("pending");
    expect(mw.blockedBy).toEqual([apiId]);

    const readyBefore = await runCli(["task", "ready", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(readyBefore).map((task) => task.id)).toEqual([apiId]);

    const claimed = await runCli(["task", "claim", apiId, "--session", "test-owner", "--json"], tmpDir);
    expect(expectJson<{ assignee: string; status: string }>(claimed)).toEqual(
      expect.objectContaining({ assignee: "test-owner", status: "pending" }),
    );

    const working = await runCli(["task", "update", apiId, "--status", "in_progress", "--json"], tmpDir);
    expect(expectJson<{ status: string }>(working).status).toBe("in_progress");

    const completed = await runCli(
      ["task", "update", apiId, "--status", "completed", "--reason", "shipped", "--json"],
      tmpDir,
    );
    const closed = expectJson<{ status: string; closeReason: string }>(completed);
    expect(closed.status).toBe("completed");
    expect(closed.closeReason).toBe("shipped");

    const readyAfter = await runCli(["task", "ready", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(readyAfter).map((task) => task.id)).toEqual([mw.id]);

    const showCompleted = await runCli(["task", "show", apiId, "--json"], tmpDir);
    expect(expectJson<{ status: string; closeReason: string }>(showCompleted)).toEqual(
      expect.objectContaining({ status: "completed", closeReason: "shipped" }),
    );
  }, SLOW_CLI_TIMEOUT_MS);

  it("rejects legacy completion and dependency commands with migration guidance", async () => {
    const id = (await runCli(["task", "q", "legacy"], tmpDir)).stdout;
    const dep = (await runCli(["task", "q", "dep"], tmpDir)).stdout;

    const badClose = await runCli(["task", "close", id], tmpDir);
    expect(badClose.exitCode).not.toBe(0);
    expect(badClose.stderr).toContain("status completed");

    const badDeps = await runCli(["task", "deps", "add", id, dep], tmpDir);
    expect(badDeps.exitCode).not.toBe(0);
    expect(badDeps.stderr).toContain("task block");
  }, SLOW_CLI_TIMEOUT_MS);

  it("supports block and unblock edits after creation", async () => {
    const blockerId = (await runCli(["task", "q", "blocker"], tmpDir)).stdout;
    const blockedId = (await runCli(["task", "q", "blocked"], tmpDir)).stdout;

    const added = await runCli(["task", "block", blockerId, blockedId, "--json"], tmpDir);
    expect(expectJson<{ blocks: string[] }>(added).blocks).toEqual([blockedId]);

    const readyBlocked = await runCli(["task", "ready", "--json"], tmpDir);
    expect(expectJson<Array<{ id: string }>>(readyBlocked).map((task) => task.id)).toEqual([blockerId]);

    const removed = await runCli(["task", "unblock", blockerId, blockedId, "--json"], tmpDir);
    expect(expectJson<{ blocks: string[] }>(removed).blocks).toEqual([]);
  }, SLOW_CLI_TIMEOUT_MS);

  it("releases unresolved tasks owned by a dead session through the recovery command", async () => {
    const id = (await runCli(["task", "q", "recover me"], tmpDir)).stdout;
    await runCli(["task", "claim", id, "--session", "dead-session", "--json"], tmpDir);
    await runCli(["task", "update", id, "--status", "in_progress", "--json"], tmpDir);

    const released = await runCli(["task", "release-owned", "dead-session", "--json"], tmpDir);
    const payload = expectJson<Array<{ id: string; status: string; assignee?: string }>>(released);
    expect(payload).toHaveLength(1);
    expect(payload[0]).toEqual(expect.objectContaining({ id, status: "pending" }));
    expect(payload[0]?.assignee).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);
});
