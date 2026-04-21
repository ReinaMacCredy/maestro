import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";

describe("task receipt on completion", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-receipt-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("attaches a receipt when --summary is passed with --status completed", async () => {
    const task = await createTask(store, { title: "ship jwt middleware" });
    const { task: completed } = await updateTask(store, task.id, {
      status: "completed",
      reason: "landed",
      summary: "added jwt middleware wrapping /api",
      surprise: "forgot refresh tokens",
      verifiedBy: ["auth.test.ts"],
    });

    expect(completed.status).toBe("completed");
    expect(completed.receipt?.summary).toBe("added jwt middleware wrapping /api");
    expect(completed.receipt?.surprise).toBe("forgot refresh tokens");
    expect(completed.receipt?.verifiedBy).toEqual(["auth.test.ts"]);
    expect(completed.receipt?.capturedAt).toBeDefined();
  });

  it("defaults summary to closeReason when summary is omitted", async () => {
    const task = await createTask(store, { title: "refactor cache" });
    const { task: completed } = await updateTask(store, task.id, {
      status: "completed",
      reason: "cache eviction fixed",
      surprise: "race in evict path",
    });

    expect(completed.receipt?.summary).toBe("cache eviction fixed");
    expect(completed.receipt?.surprise).toBe("race in evict path");
  });

  it("does not produce a receipt when no receipt fields and no reason are given", async () => {
    const task = await createTask(store, { title: "no receipt" });
    const { task: completed } = await updateTask(store, task.id, {
      status: "completed",
    });

    expect(completed.receipt).toBeUndefined();
  });

  it("rejects receipt fields when status is not completed", async () => {
    const task = await createTask(store, { title: "work in progress" });
    await expect(
      updateTask(store, task.id, {
        status: "in_progress",
        summary: "too early",
      }),
    ).rejects.toThrow(/require --status completed/);
  });

  it("clears receipt on reopen", async () => {
    const task = await createTask(store, { title: "reopened" });
    await updateTask(store, task.id, {
      status: "completed",
      summary: "first pass",
    });
    const reopened = await store.reopen(task.id);

    expect(reopened.receipt).toBeUndefined();
    expect(reopened.status).toBe("pending");
  });

  it("round-trips receipt through storage", async () => {
    const task = await createTask(store, { title: "persist" });
    await updateTask(store, task.id, {
      status: "completed",
      summary: "persisted",
      verifiedBy: ["test-a", "test-b"],
    });
    const fresh = new JsonlTaskStoreAdapter(tmpDir);
    const loaded = await fresh.get(task.id);

    expect(loaded?.receipt?.summary).toBe("persisted");
    expect(loaded?.receipt?.verifiedBy).toEqual(["test-a", "test-b"]);
  });
});
