import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";

describe("claimTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-claim-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("claims an open task and moves it to in_progress", async () => {
    const task = await createTask(store, { title: "Claim me" });

    const claimed = await claimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(claimed.assignee).toBe("codex-session-a");
    expect(claimed.claimedAt).toBeString();
    expect(claimed.status).toBe("in_progress");
  });

  it("is idempotent for the same session", async () => {
    const task = await createTask(store, { title: "Claim me" });
    const first = await claimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    const second = await claimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(second.assignee).toBe("codex-session-a");
    expect(second.status).toBe("in_progress");
    expect(second.claimedAt).toBe(first.claimedAt);
  });

  it("rejects a different session without force", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    await expect(
      claimTask(store, task.id, { sessionId: "codex-session-b" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows force-claim takeover", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const claimed = await claimTask(store, task.id, {
      sessionId: "codex-session-b",
      force: true,
    });

    expect(claimed.assignee).toBe("codex-session-b");
    expect(claimed.status).toBe("in_progress");
    expect(claimed.claimedAt).toBeString();
  });

  it("preserves blocked status when claimed", async () => {
    const task = await createTask(store, { title: "Blocked" });
    await store.update(task.id, { status: "blocked" });

    const claimed = await claimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(claimed.status).toBe("blocked");
    expect(claimed.assignee).toBe("codex-session-a");
  });

  it("rejects closed tasks", async () => {
    const task = await createTask(store, { title: "Done" });
    await store.close(task.id, { reason: "shipped" });

    await expect(
      claimTask(store, task.id, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });
});
