import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { unclaimTask } from "@/features/task/usecases/unclaim-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("unclaimTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-unclaim-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("unclaims the current owner's in_progress task back to open", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const unclaimed = await unclaimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(unclaimed.assignee).toBeUndefined();
    expect(unclaimed.claimedAt).toBeUndefined();
    expect(unclaimed.status).toBe("open");
  });

  it("preserves blocked status when unclaimed", async () => {
    const task = await createTask(store, { title: "Blocked" });
    await store.update(task.id, { status: "blocked" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const unclaimed = await unclaimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(unclaimed.status).toBe("blocked");
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
    expect(unclaimed.status).toBe("open");
  });

  it("rejects unclaiming an already unclaimed task", async () => {
    const task = await createTask(store, { title: "Open" });

    await expect(
      unclaimTask(store, task.id, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });
});
