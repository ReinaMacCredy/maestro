import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("claimTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-claim-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("claims a pending task without changing its status", async () => {
    const task = await createTask(store, { title: "Claim me" });

    const claimed = await claimTask(store, task.id, {
      sessionId: "codex-session-a",
    });

    expect(claimed.assignee).toBe("codex-session-a");
    expect(claimed.claimedAt).toBeString();
    expect(claimed.status).toBe("pending");
  });

  it("is idempotent for the same session", async () => {
    const task = await createTask(store, { title: "Claim me" });
    const first = await claimTask(store, task.id, { sessionId: "codex-session-a" });
    const second = await claimTask(store, task.id, { sessionId: "codex-session-a" });

    expect(second.assignee).toBe("codex-session-a");
    expect(second.claimedAt).toBe(first.claimedAt);
    expect(second.status).toBe("pending");
  });

  it("rejects a different session without force", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    await expect(
      claimTask(store, task.id, { sessionId: "codex-session-b" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows force takeover when blockers are clear", async () => {
    const task = await createTask(store, { title: "Claim me" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const claimed = await claimTask(store, task.id, {
      sessionId: "codex-session-b",
      force: true,
    });

    expect(claimed.assignee).toBe("codex-session-b");
    expect(claimed.status).toBe("pending");
  });

  it("rejects blocked tasks even with force", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, {
      title: "Blocked",
      blockedBy: [blocker.id],
    });

    await expect(
      claimTask(store, blocked.id, { sessionId: "codex-session-a", force: true }),
    ).rejects.toThrow(MaestroError);
  });

  it("supports optional busy-check enforcement", async () => {
    const first = await createTask(store, { title: "First" });
    const second = await createTask(store, { title: "Second" });
    await claimTask(store, first.id, { sessionId: "codex-session-a" });

    await expect(
      claimTask(store, second.id, { sessionId: "codex-session-a", checkBusy: true }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects completed tasks", async () => {
    const task = await createTask(store, { title: "Done" });
    await updateTask(store, task.id, { status: "completed", reason: "shipped" });

    await expect(
      claimTask(store, task.id, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });
});
