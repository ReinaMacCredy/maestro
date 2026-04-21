import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  FsTaskContinuationStoreAdapter,
  type TaskContinuationSummary,
} from "@/features/task/index.js";

describe("FsTaskContinuationStoreAdapter", () => {
  let tmpDir: string;
  let store: FsTaskContinuationStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-continuation-store-"));
    store = new FsTaskContinuationStoreAdapter(tmpDir);
  });

  function makeSummary(
    taskId: string,
    at: string,
    overrides: Partial<TaskContinuationSummary> = {},
  ): TaskContinuationSummary {
    return {
      taskId,
      status: "in_progress",
      lastActiveAt: at,
      currentState: `${taskId} current state`,
      nextAction: `${taskId} next action`,
      keyDecisions: [`${taskId} decision`],
      activeAgent: {
        type: "codex",
        sessionId: `${taskId}-session`,
        lastSeenAt: at,
      },
      ...overrides,
    };
  }

  it("returns no active summaries on a fresh store", async () => {
    expect(await store.listActive()).toEqual([]);
  });

  it("upserts active summaries and orders them by most recent lastActiveAt", async () => {
    const older = makeSummary("tsk-older", "2026-04-21T09:00:00.000Z");
    const newer = makeSummary("tsk-newer", "2026-04-21T10:00:00.000Z");

    await store.upsertActive(older);
    await store.upsertActive(newer);

    expect(await store.getActive("tsk-older")).toEqual(older);
    expect((await store.listActive()).map((summary) => summary.taskId)).toEqual([
      "tsk-newer",
      "tsk-older",
    ]);
  });

  it("archives a completed summary and removes it from active lookup", async () => {
    const summary = makeSummary("tsk-done", "2026-04-21T10:00:00.000Z");
    await store.upsertActive(summary);

    const archived = await store.archiveCompleted({
      ...summary,
      status: "completed",
      activeAgent: undefined,
    });

    expect(archived.status).toBe("completed");
    expect(await store.getActive("tsk-done")).toBeUndefined();
    expect(await store.getCompleted("tsk-done")).toEqual(archived);
  });

  it("reopens a completed summary back into the active set", async () => {
    const archived = await store.archiveCompleted({
      ...makeSummary("tsk-reopen", "2026-04-21T10:00:00.000Z"),
      status: "completed",
      activeAgent: undefined,
    });

    const reopened = await store.reopen("tsk-reopen", {
      ...archived,
      status: "in_progress",
      lastActiveAt: "2026-04-21T11:00:00.000Z",
      activeAgent: {
        type: "claude",
        sessionId: "claude-session",
        lastSeenAt: "2026-04-21T11:00:00.000Z",
      },
    });

    expect(reopened).toMatchObject({
      taskId: "tsk-reopen",
      status: "in_progress",
      activeAgent: {
        type: "claude",
        sessionId: "claude-session",
      },
    });
    expect(await store.getCompleted("tsk-reopen")).toBeUndefined();
    expect(await store.getActive("tsk-reopen")).toEqual(reopened);
  });

  it("lists completed summaries ordered by most recent lastActiveAt", async () => {
    const older = await store.archiveCompleted({
      ...makeSummary("tsk-old", "2026-04-20T10:00:00.000Z"),
      status: "completed",
      activeAgent: undefined,
    });
    const newer = await store.archiveCompleted({
      ...makeSummary("tsk-new", "2026-04-21T10:00:00.000Z"),
      status: "completed",
      activeAgent: undefined,
    });

    expect((await store.listCompleted()).map((summary) => summary.taskId)).toEqual([
      "tsk-new",
      "tsk-old",
    ]);
    expect(older.taskId).toBe("tsk-old");
    expect(newer.taskId).toBe("tsk-new");
  });

  it("returns no completed summaries on a fresh store", async () => {
    expect(await store.listCompleted()).toEqual([]);
  });

  it("deleteCompleted removes only the completed summary and leaves active entries untouched", async () => {
    const active = makeSummary("tsk-keep-active", "2026-04-21T10:00:00.000Z");
    await store.upsertActive(active);

    const completed = await store.archiveCompleted({
      ...makeSummary("tsk-drop", "2026-04-21T11:00:00.000Z"),
      status: "completed",
      activeAgent: undefined,
    });
    expect(await store.getCompleted("tsk-drop")).toEqual(completed);

    await store.deleteCompleted("tsk-drop");

    expect(await store.getCompleted("tsk-drop")).toBeUndefined();
    expect(await store.getActive("tsk-keep-active")).toEqual(active);
  });

  it("deleteCompleted tolerates a missing summary", async () => {
    await expect(store.deleteCompleted("tsk-absent")).resolves.toBeUndefined();
  });

  it("treats split active and completed files for the same task as repair-needed drift", async () => {
    const tasksDir = join(tmpDir, ".maestro", "tasks", "continuations");
    await mkdir(join(tasksDir, "active"), { recursive: true });
    await mkdir(join(tasksDir, "completed"), { recursive: true });
    const summary = makeSummary("tsk-conflict", "2026-04-21T10:00:00.000Z");

    await Bun.write(join(tasksDir, "active", "tsk-conflict.json"), JSON.stringify(summary, null, 2));
    await Bun.write(
      join(tasksDir, "completed", "tsk-conflict.json"),
      JSON.stringify({ ...summary, status: "completed", activeAgent: undefined }, null, 2),
    );

    await expect(store.getActive("tsk-conflict")).rejects.toThrow("repair");
  });
});
