import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { heartbeatTask } from "@/features/task/usecases/heartbeat-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";

describe("claim liveness timestamps", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-liveness-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("sets lastActivityAt when the task is claimed", async () => {
    const task = await createTask(store, { title: "claim me" });
    const claimed = await claimTask(store, task.id, { sessionId: "alpha" });

    expect(claimed.lastActivityAt).toBeString();
    expect(claimed.lastActivityAt).toBe(claimed.updatedAt);
  });

  it("bumps lastActivityAt on an owner's update but not on a force override", async () => {
    const task = await createTask(store, { title: "bump me" });
    const claimed = await claimTask(store, task.id, { sessionId: "alpha" });
    const before = claimed.lastActivityAt!;

    await new Promise((r) => setTimeout(r, 5));
    const { task: updated } = await updateTask(
      store,
      task.id,
      { description: "more detail" },
      { sessionId: "alpha" },
    );
    expect(updated.lastActivityAt).not.toBe(before);

    await new Promise((r) => setTimeout(r, 5));
    const { task: forced } = await updateTask(
      store,
      task.id,
      { description: "a different person" },
      { sessionId: "beta", force: true },
    );
    expect(forced.lastActivityAt).toBe(updated.lastActivityAt);
  });

  it("bumps lastActivityAt on heartbeat without changing other state", async () => {
    const task = await createTask(store, { title: "heartbeat me" });
    const claimed = await claimTask(store, task.id, { sessionId: "alpha" });
    const firstActivity = claimed.lastActivityAt!;

    await new Promise((r) => setTimeout(r, 5));
    const beat = await heartbeatTask(store, task.id, "alpha");

    expect(beat.lastActivityAt).not.toBe(firstActivity);
    expect(beat.status).toBe(claimed.status);
    expect(beat.assignee).toBe("alpha");
    expect(beat.claimedAt).toBe(claimed.claimedAt);
  });

  it("heartbeat rejects non-owner without force and accepts with force", async () => {
    const task = await createTask(store, { title: "guarded" });
    await claimTask(store, task.id, { sessionId: "alpha" });

    await expect(heartbeatTask(store, task.id, "intruder")).rejects.toThrow();
    await expect(heartbeatTask(store, task.id, "intruder", { force: true })).resolves.toBeDefined();
  });

  it("clears lastActivityAt on release and reopen", async () => {
    const task = await createTask(store, { title: "clear me" });
    await claimTask(store, task.id, { sessionId: "alpha" });
    await updateTask(store, task.id, { status: "in_progress" }, { sessionId: "alpha" });

    const released = await store.releaseOwned("alpha");
    expect(released[0]?.lastActivityAt).toBeUndefined();

    const task2 = await createTask(store, { title: "reopen me" });
    const claimed2 = await claimTask(store, task2.id, { sessionId: "alpha" });
    expect(claimed2.lastActivityAt).toBeString();
    await updateTask(store, task2.id, { status: "completed", reason: "done" }, { sessionId: "alpha" });
    const reopened = await store.reopen(task2.id);
    expect(reopened.lastActivityAt).toBeUndefined();
  });
});
