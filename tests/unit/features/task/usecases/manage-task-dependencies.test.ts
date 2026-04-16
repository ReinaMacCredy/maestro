import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { blockTasks, unblockTasks } from "@/features/task/usecases/manage-task-dependencies.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("manage blocker edges", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-block-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("adds reciprocal blocker edges after creation", async () => {
    const blocker = await createTask(store, { title: "A" });
    const blockedA = await createTask(store, { title: "B" });
    const blockedB = await createTask(store, { title: "C" });

    const updated = await blockTasks(store, blocker.id, [blockedA.id, blockedB.id]);

    expect(updated.blocks).toEqual([blockedA.id, blockedB.id]);
    expect((await store.get(blockedA.id))?.blockedBy).toEqual([blocker.id]);
    expect((await store.get(blockedB.id))?.blockedBy).toEqual([blocker.id]);
  });

  it("rejects self-block cycles", async () => {
    const task = await createTask(store, { title: "Main" });

    await expect(
      blockTasks(store, task.id, [task.id]),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects transitive blocker cycles", async () => {
    const taskA = await createTask(store, { title: "A" });
    const taskB = await createTask(store, { title: "B" });
    await blockTasks(store, taskA.id, [taskB.id]);

    await expect(
      blockTasks(store, taskB.id, [taskA.id]),
    ).rejects.toThrow(MaestroError);
  });

  it("removes blocker edges idempotently", async () => {
    const blocker = await createTask(store, { title: "A" });
    const blocked = await createTask(store, { title: "B" });
    await blockTasks(store, blocker.id, [blocked.id]);

    const once = await unblockTasks(store, blocker.id, [blocked.id]);
    const twice = await unblockTasks(store, blocker.id, [blocked.id]);

    expect(once.blocks).toEqual([]);
    expect(twice.blocks).toEqual([]);
    expect((await store.get(blocked.id))?.blockedBy).toEqual([]);
  });
});
