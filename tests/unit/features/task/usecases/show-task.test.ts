import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { showTask } from "@/features/task/usecases/show-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("showTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-show-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("returns an existing task", async () => {
    const created = await createTask(store, { title: "Viewed" });
    const loaded = await showTask(store, created.id);
    expect(loaded.id).toBe(created.id);
    expect(loaded.title).toBe("Viewed");
  });

  it("throws taskNotFound for unknown id", async () => {
    await expect(showTask(store, "tsk-000000")).rejects.toThrow(MaestroError);
  });
});
