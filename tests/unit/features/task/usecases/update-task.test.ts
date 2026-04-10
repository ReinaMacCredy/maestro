import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
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
    const updated = await updateTask(store, task.id, {
      patch: { title: "New", priority: 1 },
    });
    expect(updated.title).toBe("New");
    expect(updated.priority).toBe(1);
  });

  it("rejects unknown id", async () => {
    await expect(
      updateTask(store, "tsk-000000", { patch: { title: "X" } }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects --status closed (must use close)", async () => {
    const task = await createTask(store, { title: "Done" });
    await expect(
      updateTask(store, task.id, { patch: { status: "closed" } }),
    ).rejects.toThrow(MaestroError);
  });

  it("accepts other status transitions", async () => {
    const task = await createTask(store, { title: "Doing" });
    const updated = await updateTask(store, task.id, {
      patch: { status: "in_progress" },
    });
    expect(updated.status).toBe("in_progress");
  });

  it("applies --claim to set assignee and status atomically", async () => {
    const task = await createTask(store, { title: "Claim me" });
    const updated = await updateTask(store, task.id, {
      patch: {},
      claim: { sessionId: "claude-code-abc123" },
    });
    expect(updated.assignee).toBe("claude-code-abc123");
    expect(updated.status).toBe("in_progress");
  });

  it("--claim overrides an explicit assignee in the patch", async () => {
    const task = await createTask(store, { title: "Claim" });
    const updated = await updateTask(store, task.id, {
      patch: { assignee: "someone-else" },
      claim: { sessionId: "winner" },
    });
    expect(updated.assignee).toBe("winner");
    expect(updated.status).toBe("in_progress");
  });

  it("rejects parenting under an unknown task", async () => {
    const task = await createTask(store, { title: "Orphan" });
    await expect(
      updateTask(store, task.id, { patch: { parentId: "tsk-000000" } }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects parenting that would create a cycle", async () => {
    const root = await createTask(store, { title: "Root" });
    const mid = await createTask(store, { title: "Mid", parentId: root.id });
    const leaf = await createTask(store, { title: "Leaf", parentId: mid.id });

    // Trying to parent root under leaf creates: leaf -> mid -> root -> leaf
    await expect(
      updateTask(store, root.id, { patch: { parentId: leaf.id } }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows reparenting within a valid tree", async () => {
    const a = await createTask(store, { title: "A" });
    const b = await createTask(store, { title: "B" });
    const leaf = await createTask(store, { title: "leaf", parentId: a.id });

    const moved = await updateTask(store, leaf.id, { patch: { parentId: b.id } });
    expect(moved.parentId).toBe(b.id);
  });

  it("allows clearing parent via empty string", async () => {
    const root = await createTask(store, { title: "Root" });
    const child = await createTask(store, { title: "Child", parentId: root.id });
    const cleared = await updateTask(store, child.id, {
      patch: { parentId: "" },
    });
    expect(cleared.parentId).toBeUndefined();
  });

  it("adds and removes labels", async () => {
    const task = await createTask(store, { title: "L", labels: ["a"] });
    const added = await updateTask(store, task.id, {
      patch: { addLabels: ["b", "c"] },
    });
    expect(added.labels).toEqual(["a", "b", "c"]);

    const removed = await updateTask(store, task.id, {
      patch: { removeLabels: ["a"] },
    });
    expect(removed.labels).toEqual(["b", "c"]);
  });
});
