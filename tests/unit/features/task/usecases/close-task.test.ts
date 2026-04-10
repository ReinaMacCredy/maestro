import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { closeTask } from "@/features/task/usecases/close-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("closeTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-close-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("closes a task with a reason", async () => {
    const task = await createTask(store, { title: "Done" });
    const closed = await closeTask(store, task.id, { reason: "shipped" });
    expect(closed.status).toBe("closed");
    expect(closed.closeReason).toBe("shipped");
  });

  it("closes without a reason", async () => {
    const task = await createTask(store, { title: "Done" });
    const closed = await closeTask(store, task.id, {});
    expect(closed.status).toBe("closed");
    expect(closed.closeReason).toBeUndefined();
  });

  it("throws taskNotFound for unknown id", async () => {
    await expect(closeTask(store, "tsk-000000", {})).rejects.toThrow(MaestroError);
  });
});
