import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("updateTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-update-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("updates basic fields", async () => {
    const task = await createTask(store, { title: "Original" });
    const updated = await updateTask(store, task.id, { title: "New", priority: 1 });

    expect(updated.title).toBe("New");
    expect(updated.priority).toBe(1);
  });

  it("rejects moving an unclaimed task to in_progress", async () => {
    const task = await createTask(store, { title: "Doing" });

    await expect(
      updateTask(store, task.id, { status: "in_progress" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows a claimed task to move to in_progress", async () => {
    const task = await createTask(store, { title: "Doing" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const updated = await updateTask(store, task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });
    expect(updated.status).toBe("in_progress");
  });

  it("rejects moving a claimed task back to pending via update", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });
    await updateTask(store, task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

    await expect(
      updateTask(store, task.id, { status: "pending" }, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows same-owner metadata edits while a task remains claimed and pending", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const updated = await updateTask(
      store,
      task.id,
      { title: "Retitled while pending" },
      { sessionId: "codex-session-a" },
    );

    expect(updated.title).toBe("Retitled while pending");
    expect(updated.status).toBe("pending");
  });

  it("rejects claimed-task edits without the owning session", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    await expect(
      updateTask(store, task.id, { title: "Nope" }),
    ).rejects.toThrow(MaestroError);

    await expect(
      updateTask(store, task.id, { title: "Still nope" }, { sessionId: "codex-session-b" }),
    ).rejects.toThrow(MaestroError);
  });

  it("completes a task with a reason", async () => {
    const task = await createTask(store, { title: "Done" });

    const completed = await updateTask(store, task.id, {
      status: "completed",
      reason: "shipped",
    });

    expect(completed.status).toBe("completed");
    expect(completed.closeReason).toBe("shipped");
  });

  it("rejects completing a task while unresolved blockers remain", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, { title: "Blocked", blockedBy: [blocker.id] });

    await expect(
      updateTask(store, blocked.id, { status: "completed", reason: "skipped ahead" }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects completion reasons without completed status", async () => {
    const task = await createTask(store, { title: "Oops" });

    await expect(
      updateTask(store, task.id, { reason: "not yet" }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects edits to completed tasks", async () => {
    const task = await createTask(store, { title: "Done" });
    await updateTask(store, task.id, { status: "completed", reason: "shipped" });

    await expect(
      updateTask(store, task.id, { title: "still mutable?" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows reparenting within a valid tree", async () => {
    const a = await createTask(store, { title: "A" });
    const b = await createTask(store, { title: "B" });
    const leaf = await createTask(store, { title: "leaf", parentId: a.id });

    const moved = await updateTask(store, leaf.id, { parentId: b.id });
    expect(moved.parentId).toBe(b.id);
  });
});
