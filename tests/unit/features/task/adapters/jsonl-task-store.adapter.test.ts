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
    const working = await store.update(task.id, { status: "in_progress" });
    expect(working.status).toBe("in_progress");
    await expect(store.update(task.id, { status: "pending" })).rejects.toThrow(MaestroError);
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
    await store.update(task.id, { status: "in_progress" });

    const unclaimed = await store.unclaim(task.id, "codex-session-a");
    expect(unclaimed.status).toBe("pending");
    expect(unclaimed.assignee).toBeUndefined();
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
    const completed = await store.update(task.id, { status: "completed", reason: "shipped" });

    expect(completed.status).toBe("completed");
    expect(completed.closeReason).toBe("shipped");

    const fresh = new JsonlTaskStoreAdapter(tmpDir);
    expect((await fresh.get(task.id))?.closeReason).toBe("shipped");
  });

  it("releases unresolved tasks owned by a dead session", async () => {
    const task = await store.create({ title: "Owned" });
    await store.claim(task.id, "codex-session-a");
    await store.update(task.id, { status: "in_progress" });

    const released = await store.releaseOwned("codex-session-a");
    expect(released).toHaveLength(1);
    expect(released[0]?.status).toBe("pending");
    expect(released[0]?.assignee).toBeUndefined();
  });
});
