import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
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

    const working = await runCli(
      ["task", "update", apiId, "--status", "in_progress", "--session", "test-owner", "--json"],
      tmpDir,
    );
    expect(expectJson<{ status: string }>(working).status).toBe("in_progress");

    const completed = await runCli(
      ["task", "update", apiId, "--status", "completed", "--reason", "shipped", "--session", "test-owner", "--json"],
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
    await runCli(["task", "update", id, "--status", "in_progress", "--session", "dead-session", "--json"], tmpDir);

    const released = await runCli(["task", "release-owned", "dead-session", "--json"], tmpDir);
    const payload = expectJson<Array<{ id: string; status: string; assignee?: string }>>(released);
    expect(payload).toHaveLength(1);
    expect(payload[0]).toEqual(expect.objectContaining({ id, status: "pending" }));
    expect(payload[0]?.assignee).toBeUndefined();
  }, SLOW_CLI_TIMEOUT_MS);

  it("syncs continuation state when ready auto-releases a stale owner", async () => {
    const id = (await runCli(["task", "q", "stale owner continuation"], tmpDir)).stdout;
    await runCli(["task", "claim", id, "--session", "codex-stale-session", "--json"], tmpDir);
    await runCli(
      [
        "task",
        "update",
        id,
        "--status",
        "in_progress",
        "--session",
        "codex-stale-session",
        "--current-state",
        "Actively investigating before the shell disappeared.",
        "--json",
      ],
      tmpDir,
    );

    const ready = await runCli(
      ["task", "ready", "--json"],
      tmpDir,
      {
        env: {
          MAESTRO_CODEX_SESSIONS_DIR: join(tmpDir, "missing-codex-sessions"),
          MAESTRO_CLAUDE_SESSIONS_DIR: join(tmpDir, "missing-claude-sessions"),
          MAESTRO_CLAUDE_PROJECTS_DIR: join(tmpDir, "missing-claude-projects"),
        },
      },
    );
    expect(ready.exitCode).toBe(0);
    expect(expectJson<Array<{ id: string }>>(ready).map((task) => task.id)).toContain(id);

    const summaryPath = join(tmpDir, ".maestro", "tasks", "continuations", "active", `${id}.json`);
    const summary = JSON.parse(await readFile(summaryPath, "utf8")) as {
      status: string;
      currentState: string;
      activeAgent?: unknown;
    };
    expect(summary.status).toBe("pending");
    expect(summary.currentState).toBe("Task ownership released back to the queue.");
    expect(summary.activeAgent).toBeUndefined();

    const historyPath = join(tmpDir, ".maestro", "tasks", "local-history", `${id}.jsonl`);
    const history = await readFile(historyPath, "utf8");
    expect(history).toContain("Recovered from stale owner codex-stale-session");
  }, SLOW_CLI_TIMEOUT_MS);

  it("writes a continuation summary on active work and archives it on completion", async () => {
    const id = (await runCli(["task", "q", "continue me"], tmpDir)).stdout;
    await runCli(["task", "claim", id, "--session", "codex-session-a", "--json"], tmpDir);
    await runCli(["task", "update", id, "--status", "in_progress", "--session", "codex-session-a", "--json"], tmpDir);

    const activeSummaryPath = join(tmpDir, ".maestro", "tasks", "continuations", "active", `${id}.json`);
    const localHistoryPath = join(tmpDir, ".maestro", "tasks", "local-history", `${id}.jsonl`);
    const activeSummary = JSON.parse(await readFile(activeSummaryPath, "utf8")) as {
      status: string;
      activeAgent?: { type: string; sessionId?: string };
    };
    expect(activeSummary).toMatchObject({
      status: "in_progress",
      activeAgent: {
        type: "codex",
        sessionId: "session-a",
      },
    });

    const history = await readFile(localHistoryPath, "utf8");
    expect(history).toContain("\"kind\":\"snapshot\"");

    await runCli(
      ["task", "update", id, "--status", "completed", "--reason", "done", "--session", "codex-session-a", "--json"],
      tmpDir,
    );

    await expect(access(activeSummaryPath)).rejects.toBeDefined();
    const completedSummary = JSON.parse(
      await readFile(join(tmpDir, ".maestro", "tasks", "continuations", "completed", `${id}.json`), "utf8"),
    ) as { status: string };
    expect(completedSummary.status).toBe("completed");
  }, SLOW_CLI_TIMEOUT_MS);

  it("renders the continuation summary even when local timeline history is missing", async () => {
    const created = await runCli(["task", "create", "show me", "--json"], tmpDir);
    const task = expectJson<{ id: string }>(created);
    const summaryPath = join(tmpDir, ".maestro", "tasks", "continuations", "active", `${task.id}.json`);
    await Bun.write(
      summaryPath,
      JSON.stringify({
        taskId: task.id,
        status: "in_progress",
        lastActiveAt: "2026-04-21T10:00:00.000Z",
        currentState: "Context restored from summary only",
        nextAction: "Keep going from the saved next step",
        keyDecisions: ["Do not touch the session store"],
        activeAgent: {
          type: "codex",
          sessionId: "session-a",
          lastSeenAt: "2026-04-21T10:00:00.000Z",
        },
      }, null, 2),
    );

    const shown = await runCli(["task", "show", task.id], tmpDir);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("Current state: Context restored from summary only");
    expect(shown.stdout).toContain("Next action: Keep going from the saved next step");
    expect(shown.stdout).toContain("Recent timeline: no local timeline available");
  }, SLOW_CLI_TIMEOUT_MS);

  it("updates explicit continuation state and active decisions through task update", async () => {
    const id = (await runCli(["task", "q", "resume me"], tmpDir)).stdout;
    await runCli(["task", "claim", id, "--session", "codex-session-a", "--json"], tmpDir);
    await runCli(["task", "update", id, "--status", "in_progress", "--session", "codex-session-a", "--json"], tmpDir);

    await runCli(
      [
        "task",
        "update",
        id,
        "--session",
        "codex-session-a",
        "--current-state",
        "JWT parsing is fixed; admin role mapping still fails.",
        "--next-action",
        "Patch role mapping and rerun auth tests.",
        "--add-decision",
        "Keep middleware signature unchanged,Do not touch the session store",
        "--json",
      ],
      tmpDir,
    );

    const summaryPath = join(tmpDir, ".maestro", "tasks", "continuations", "active", `${id}.json`);
    const localHistoryPath = join(tmpDir, ".maestro", "tasks", "local-history", `${id}.jsonl`);
    const summary = JSON.parse(await readFile(summaryPath, "utf8")) as {
      currentState: string;
      nextAction: string;
      keyDecisions: string[];
    };
    expect(summary).toMatchObject({
      currentState: "JWT parsing is fixed; admin role mapping still fails.",
      nextAction: "Patch role mapping and rerun auth tests.",
      keyDecisions: [
        "Keep middleware signature unchanged",
        "Do not touch the session store",
      ],
    });

    const history = await readFile(localHistoryPath, "utf8");
    expect(history).toContain("\"kind\":\"snapshot\"");
    expect(history).toContain("\"kind\":\"next_action_set\"");
    expect(history).toContain("\"kind\":\"decision\"");

    await runCli(
      [
        "task",
        "update",
        id,
        "--session",
        "codex-session-a",
        "--remove-decision",
        "Do not touch the session store",
        "--json",
      ],
      tmpDir,
    );
    const narrowed = JSON.parse(await readFile(summaryPath, "utf8")) as {
      keyDecisions: string[];
    };
    expect(narrowed.keyDecisions).toEqual(["Keep middleware signature unchanged"]);
  }, SLOW_CLI_TIMEOUT_MS);
});
