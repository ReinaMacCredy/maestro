import { beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { TASK_ID_PATTERN } from "@/features/task/domain/task-id.js";
import { MaestroError } from "@/shared/errors.js";

describe("JsonlTaskStoreAdapter", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-adapter-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("creates tasks with defaults and generated ids", async () => {
    const task = await store.create({ title: "First task" });

    expect(task.id).toMatch(TASK_ID_PATTERN);
    expect(task.status).toBe("pending");
    expect(task.blocks).toEqual([]);
    expect(task.blockedBy).toEqual([]);
  });

  it("persists tasks across store instances", async () => {
    const created = await store.create({ title: "Persist me" });

    const fresh = new JsonlTaskStoreAdapter(tmpDir);
    expect((await fresh.get(created.id))?.title).toBe("Persist me");
  });

  it("normalizes legacy rows on read without rewriting them", async () => {
    const tasksDir = join(tmpDir, ".maestro", "tasks");
    const jsonlPath = join(tasksDir, "tasks.jsonl");
    await mkdir(tasksDir, { recursive: true });
    const legacyRow = JSON.stringify({
      id: "tsk-abc123",
      title: "Legacy",
      type: "task",
      priority: 2,
      status: "open",
      labels: [],
      dependsOn: ["tsk-000001"],
      createdAt: "2026-04-12T00:00:00.000Z",
      updatedAt: "2026-04-12T00:00:00.000Z",
    });
    const blockerRow = JSON.stringify({
      id: "tsk-000001",
      title: "Blocker",
      type: "task",
      priority: 2,
      status: "pending",
      labels: [],
      blocks: [],
      blockedBy: [],
      createdAt: "2026-04-12T00:00:01.000Z",
      updatedAt: "2026-04-12T00:00:01.000Z",
    });
    await Bun.write(jsonlPath, `${legacyRow}\n${blockerRow}\n`);

    const loaded = await store.get("tsk-abc123");
    const rawAfterRead = await readFile(jsonlPath, "utf8");

    expect(loaded?.status).toBe("pending");
    expect(loaded?.blockedBy).toEqual(["tsk-000001"]);
    expect(rawAfterRead.trim()).toBe(`${legacyRow}\n${blockerRow}`);
  });

  it("preserves orphan blocker references across unrelated writes", async () => {
    const tasksDir = join(tmpDir, ".maestro", "tasks");
    const jsonlPath = join(tasksDir, "tasks.jsonl");
    await mkdir(tasksDir, { recursive: true });
    await Bun.write(
      jsonlPath,
      `${JSON.stringify({
        id: "tsk-0f0f0f",
        title: "Legacy",
        type: "task",
        priority: 2,
        status: "pending",
        labels: [],
        blocks: ["tsk-feed02"],
        blockedBy: ["tsk-dead01"],
        createdAt: "2026-04-12T00:00:00.000Z",
        updatedAt: "2026-04-12T00:00:00.000Z",
      })}\n`,
    );

    const loaded = await store.get("tsk-0f0f0f");
    expect(loaded?.blocks).toEqual(["tsk-feed02"]);
    expect(loaded?.blockedBy).toEqual(["tsk-dead01"]);

    await store.update("tsk-0f0f0f", { title: "Still blocked" });

    const rewritten = JSON.parse((await readFile(jsonlPath, "utf8")).trim()) as {
      blocks: string[];
      blockedBy: string[];
      title: string;
    };
    expect(rewritten.title).toBe("Still blocked");
    expect(rewritten.blocks).toEqual(["tsk-feed02"]);
    expect(rewritten.blockedBy).toEqual(["tsk-dead01"]);
  });

  it("creates reciprocal blocker edges", async () => {
    const blocker = await store.create({ title: "Blocker" });
    const blocked = await store.create({ title: "Blocked", blockedBy: [blocker.id] });

    expect(blocked.blockedBy).toEqual([blocker.id]);
    expect((await store.get(blocker.id))?.blocks).toEqual([blocked.id]);
  });

    it("updates tasks while enforcing the new status invariants", async () => {
      const task = await store.create({ title: "Doing" });

      await expect(store.update(task.id, { status: "in_progress" })).rejects.toThrow(MaestroError);
      await store.claim(task.id, "codex-session-a");
      const { task: working } = await store.update(task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });
      expect(working.status).toBe("in_progress");
      await expect(
        store.update(task.id, { status: "pending" }, { sessionId: "codex-session-a" }),
      ).rejects.toThrow(MaestroError);
    });

  it("claims ownership without changing status", async () => {
    const task = await store.create({ title: "Claim me" });
    const claimed = await store.claim(task.id, "codex-session-a");

    expect(claimed.assignee).toBe("codex-session-a");
    expect(claimed.status).toBe("pending");
  });

  it("blocks claim when unresolved blockers exist", async () => {
    const blocker = await store.create({ title: "Blocker" });
    const blocked = await store.create({ title: "Blocked", blockedBy: [blocker.id] });

    await expect(store.claim(blocked.id, "codex-session-a")).rejects.toThrow(MaestroError);
  });

  it("enforces optional busy-check ownership", async () => {
    const first = await store.create({ title: "First" });
    const second = await store.create({ title: "Second" });
    await store.claim(first.id, "codex-session-a");

    await expect(
      store.claim(second.id, "codex-session-a", { checkBusy: true }),
    ).rejects.toThrow(MaestroError);
  });

    it("unclaims in-progress work back to pending", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");
      await store.update(task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

      const unclaimed = await store.unclaim(task.id, "codex-session-a");
      expect(unclaimed.status).toBe("pending");
      expect(unclaimed.assignee).toBeUndefined();
    });

    it("allows same-owner metadata edits while a task stays pending", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");

      const { task: updated } = await store.update(
        task.id,
        { title: "Retitled" },
        { sessionId: "codex-session-a" },
      );

      expect(updated.title).toBe("Retitled");
      expect(updated.status).toBe("pending");
    });

    it("rejects claimed-task mutations without the owner context", async () => {
      const blocker = await store.create({ title: "Blocker" });
      const blocked = await store.create({ title: "Blocked" });
      await store.claim(blocker.id, "codex-session-a");
      await store.claim(blocked.id, "codex-session-a");

      await expect(store.update(blocker.id, { title: "Nope" })).rejects.toThrow(MaestroError);
      await expect(
        store.update(blocker.id, { title: "Nope" }, { sessionId: "codex-session-b" }),
      ).rejects.toThrow(MaestroError);
      await expect(store.block(blocker.id, [blocked.id])).rejects.toThrow(MaestroError);
      await expect(
        store.block(blocker.id, [blocked.id], { sessionId: "codex-session-b" }),
      ).rejects.toThrow(MaestroError);
      await expect(store.unblock(blocker.id, [blocked.id])).rejects.toThrow(MaestroError);
      await expect(
        store.unblock(blocker.id, [blocked.id], { sessionId: "codex-session-b" }),
      ).rejects.toThrow(MaestroError);
    });

  it("normalizes same-owner legacy claimed rows into canonical claimed state", async () => {
    const tasksDir = join(tmpDir, ".maestro", "tasks");
    await mkdir(tasksDir, { recursive: true });
    await Bun.write(
      join(tasksDir, "tasks.jsonl"),
      `${JSON.stringify({
        id: "tsk-abc123",
        title: "Legacy",
        type: "task",
        priority: 2,
        status: "open",
        labels: [],
        blocks: [],
        blockedBy: [],
        assignee: "codex-legacy",
        createdAt: "2026-04-12T00:00:00.000Z",
        updatedAt: "2026-04-12T00:00:00.000Z",
      })}\n`,
    );

    const claimed = await store.claim("tsk-abc123", "codex-legacy");
    expect(claimed.assignee).toBe("codex-legacy");
    expect(claimed.claimedAt).toBeString();
    expect(claimed.status).toBe("pending");
  });

  it("adds and removes blocker edges idempotently", async () => {
    const blocker = await store.create({ title: "A" });
    const blocked = await store.create({ title: "B" });
    const second = await store.create({ title: "C" });

    const updated = await store.block(blocker.id, [blocked.id, second.id]);
    expect(updated.blocks).toEqual([blocked.id, second.id]);
    expect((await store.get(blocked.id))?.blockedBy).toEqual([blocker.id]);

    const once = await store.unblock(blocker.id, [blocked.id]);
    const twice = await store.unblock(blocker.id, [blocked.id]);
    expect(once.blocks).toEqual([second.id]);
    expect(twice.blocks).toEqual([second.id]);
    expect((await store.get(blocked.id))?.blockedBy).toEqual([]);
  });

  it("completes tasks through update and persists close reasons", async () => {
    const task = await store.create({ title: "Done" });
    const { task: completed } = await store.update(task.id, { status: "completed", reason: "shipped" });

    expect(completed.status).toBe("completed");
    expect(completed.closeReason).toBe("shipped");

    const fresh = new JsonlTaskStoreAdapter(tmpDir);
    expect((await fresh.get(task.id))?.closeReason).toBe("shipped");
  });

  it("syncs internal task metadata without widening task update", async () => {
    const task = await store.create({ title: "Meta" });

    const updated = await store.syncMetadata(task.id, {
      contractId: "c-a1b2c3",
      claimedAtCommit: "0123456789abcdef0123456789abcdef01234567",
    });

    expect(updated.contractId).toBe("c-a1b2c3");
    expect(updated.claimedAtCommit).toBe("0123456789abcdef0123456789abcdef01234567");
    expect(updated.updatedAt).toBe(task.updatedAt);
  });

  it("deletes a task and removes blocker and parent references from the remaining graph", async () => {
    const blocker = await store.create({ title: "Blocker" });
    const target = await store.create({ title: "Target", blockedBy: [blocker.id] });
    const child = await store.create({ title: "Child", parentId: target.id });

    const deleted = await store.delete(target.id);
    expect(deleted.id).toBe(target.id);
    expect(await store.get(target.id)).toBeUndefined();
    expect((await store.get(blocker.id))?.blocks).toEqual([]);
    expect((await store.get(child.id))?.parentId).toBeUndefined();
  });

    it("releases unresolved tasks owned by a dead session", async () => {
      const task = await store.create({ title: "Owned" });
      await store.claim(task.id, "codex-session-a");
      await store.update(task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

      const released = await store.releaseOwned("codex-session-a");
      expect(released).toHaveLength(1);
    expect(released[0]?.status).toBe("pending");
    expect(released[0]?.assignee).toBeUndefined();
  });

  it("reopen clears claimedAtCommit along with other claim-scoped state", async () => {
    const task = await store.create({ title: "Reopen me" });
    await store.claim(task.id, "session-a");
    await store.syncMetadata(task.id, { claimedAtCommit: "deadbeef" });
    await store.update(
      task.id,
      { status: "completed", reason: "done" },
      { sessionId: "session-a" },
    );

    const reopened = await store.reopen(task.id);
    expect(reopened.status).toBe("pending");
    expect(reopened.assignee).toBeUndefined();
    expect(reopened.claimedAt).toBeUndefined();
    expect(reopened.claimedAtCommit).toBeUndefined();
    // contractId is intentionally preserved so the reopen flow can re-lock
    // the prior contract for the task.
  });
});
