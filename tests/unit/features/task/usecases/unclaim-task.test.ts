import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { unclaimTask } from "@/features/task/usecases/unclaim-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("unclaimTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-unclaim-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("unclaims the current owner and resets in-progress work to pending", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });
    await updateTask(store, task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

    const unclaimed = await unclaimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(unclaimed.assignee).toBeUndefined();
    expect(unclaimed.claimedAt).toBeUndefined();
    expect(unclaimed.status).toBe("pending");
  });

  it("preserves pending status when ownership is released before work starts", async () => {
    const task = await createTask(store, { title: "Pending" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const unclaimed = await unclaimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(unclaimed.status).toBe("pending");
    expect(unclaimed.assignee).toBeUndefined();
  });

  it("rejects unclaim by a different session without force", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    await expect(
      unclaimTask(store, task.id, { sessionId: "codex-session-b" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows force-unclaim by another session", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const unclaimed = await unclaimTask(store, task.id, {
      sessionId: "codex-session-b",
      force: true,
    });

    expect(unclaimed.assignee).toBeUndefined();
    expect(unclaimed.status).toBe("pending");
  });

  it("rejects already-unclaimed tasks", async () => {
    const task = await createTask(store, { title: "Open" });

    await expect(
      unclaimTask(store, task.id, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });
});
