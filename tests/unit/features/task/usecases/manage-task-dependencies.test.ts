import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { addTaskDependencies, removeTaskDependencies } from "@/features/task/usecases/manage-task-dependencies.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("manageTaskDependencies", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-deps-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("adds dependencies after task creation", async () => {
    const depA = await createTask(store, { title: "A" });
    const depB = await createTask(store, { title: "B" });
    const task = await createTask(store, { title: "Main" });

    const updated = await addTaskDependencies(store, task.id, [depA.id, depB.id]);

    expect(updated.dependsOn).toEqual([depA.id, depB.id]);
  });

  it("deduplicates repeated dependencies while preserving order", async () => {
    const depA = await createTask(store, { title: "A" });
    const depB = await createTask(store, { title: "B" });
    const task = await createTask(store, { title: "Main", dependsOn: [depA.id] });

    const updated = await addTaskDependencies(store, task.id, [depA.id, depB.id, depA.id]);

    expect(updated.dependsOn).toEqual([depA.id, depB.id]);
  });

  it("rejects self-dependencies", async () => {
    const task = await createTask(store, { title: "Main" });

    await expect(
      addTaskDependencies(store, task.id, [task.id]),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects transitive dependency cycles", async () => {
    const taskA = await createTask(store, { title: "A" });
    const taskB = await createTask(store, { title: "B", dependsOn: [taskA.id] });

    await expect(
      addTaskDependencies(store, taskA.id, [taskB.id]),
    ).rejects.toThrow(MaestroError);
  });

  it("removes dependencies idempotently", async () => {
    const depA = await createTask(store, { title: "A" });
    const depB = await createTask(store, { title: "B" });
    const task = await createTask(store, { title: "Main", dependsOn: [depA.id, depB.id] });

    const once = await removeTaskDependencies(store, task.id, [depA.id]);
    expect(once.dependsOn).toEqual([depB.id]);

    const twice = await removeTaskDependencies(store, task.id, [depA.id]);
    expect(twice.dependsOn).toEqual([depB.id]);
  });
});
