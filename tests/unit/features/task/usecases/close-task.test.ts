import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { closeTask } from "@/features/task/usecases/close-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("closeTask compatibility alias", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-close-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("completes a task with a reason", async () => {
    const task = await createTask(store, { title: "Done" });
    const closed = await closeTask(store, task.id, { reason: "shipped" });

    expect(closed.status).toBe("completed");
    expect(closed.closeReason).toBe("shipped");
  });

  it("rejects re-closing an already completed task", async () => {
    const task = await createTask(store, { title: "Done" });
    await closeTask(store, task.id, { reason: "shipped" });

    await expect(closeTask(store, task.id, { reason: "retry" })).rejects.toThrow(MaestroError);
  });
});
