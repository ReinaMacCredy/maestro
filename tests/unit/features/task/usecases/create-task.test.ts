import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("createTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-create-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("creates a task with minimal input", async () => {
    const task = await createTask(store, { title: "First" });
    expect(task.title).toBe("First");
    expect(task.status).toBe("open");
    expect(task.priority).toBe(2);
  });

  it("rejects empty title", async () => {
    await expect(createTask(store, { title: "" })).rejects.toThrow(MaestroError);
    await expect(createTask(store, { title: "   " })).rejects.toThrow(MaestroError);
  });

  it("rejects invalid priority", async () => {
    await expect(
      createTask(store, { title: "X", priority: 9 as never }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects unknown dependency ids", async () => {
    await expect(
      createTask(store, { title: "X", dependsOn: ["tsk-000000"] }),
    ).rejects.toThrow(MaestroError);
  });

  it("accepts dependency on an existing task", async () => {
    const parent = await createTask(store, { title: "Parent" });
    const child = await createTask(store, {
      title: "Child",
      dependsOn: [parent.id],
    });
    expect(child.dependsOn).toEqual([parent.id]);
  });

  it("rejects unknown parent id", async () => {
    await expect(
      createTask(store, { title: "Orphan", parentId: "tsk-000000" }),
    ).rejects.toThrow(MaestroError);
  });

  it("accepts existing parent id", async () => {
    const parent = await createTask(store, { title: "Root" });
    const child = await createTask(store, {
      title: "Leaf",
      parentId: parent.id,
    });
    expect(child.parentId).toBe(parent.id);
  });

  it("trims the title", async () => {
    const task = await createTask(store, { title: "  spaced  " });
    expect(task.title).toBe("spaced");
  });

  it("respects labels and dedups nothing (first occurrence preserved)", async () => {
    const task = await createTask(store, {
      title: "Labeled",
      labels: ["auth", "urgent"],
    });
    expect(task.labels).toEqual(["auth", "urgent"]);
  });
});
