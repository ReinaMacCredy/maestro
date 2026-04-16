import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, stat, utimes } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { MaestroError } from "@/shared/errors.js";
import { TASK_ID_PATTERN } from "@/features/task/domain/task-id.js";

describe("JsonlTaskStoreAdapter", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-adapter-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  describe("create", () => {
    it("creates a task with a generated id and defaults", async () => {
      const task = await store.create({ title: "First task" });

      expect(task.id).toMatch(TASK_ID_PATTERN);
      expect(task.title).toBe("First task");
      expect(task.type).toBe("task");
      expect(task.priority).toBe(2);
      expect(task.status).toBe("open");
      expect(task.labels).toEqual([]);
      expect(task.dependsOn).toEqual([]);
      expect(task.createdAt).toBeString();
      expect(task.updatedAt).toBe(task.createdAt);
    });

    it("persists created tasks to disk across instances", async () => {
      const created = await store.create({ title: "Persist me" });

      const fresh = new JsonlTaskStoreAdapter(tmpDir);
      const loaded = await fresh.get(created.id);
      expect(loaded).toBeDefined();
      expect(loaded?.title).toBe("Persist me");
    });

    it("respects provided priority, type, labels, depends-on", async () => {
      const dependency = await store.create({ title: "Dependency" });
      const task = await store.create({
        title: "Custom",
        priority: 0,
        type: "bug",
        labels: ["urgent", "auth"],
        dependsOn: [dependency.id],
      });

      expect(task.priority).toBe(0);
      expect(task.type).toBe("bug");
      expect(task.labels).toEqual(["urgent", "auth"]);
      expect(task.dependsOn).toEqual([dependency.id]);
    });

    it("creates distinct ids for sequential tasks", async () => {
      const a = await store.create({ title: "A" });
      const b = await store.create({ title: "B" });
      const c = await store.create({ title: "C" });
      expect(new Set([a.id, b.id, c.id]).size).toBe(3);
    });
  });

  describe("get / all", () => {
    it("returns undefined for unknown id", async () => {
      expect(await store.get("tsk-000000")).toBeUndefined();
    });

    it("returns all created tasks", async () => {
      await store.create({ title: "A" });
      await store.create({ title: "B" });
      await store.create({ title: "C" });
      const tasks = await store.all();
      expect(tasks.length).toBe(3);
    });

    it("returns empty array on a fresh store", async () => {
      const tasks = await store.all();
      expect(tasks).toEqual([]);
    });

    it("surfaces malformed task storage instead of dropping bad lines", async () => {
      const tasksDir = join(tmpDir, ".maestro", "tasks");
      await mkdir(tasksDir, { recursive: true });
      await Bun.write(join(tasksDir, "tasks.jsonl"), "{\"id\":\n");

      await expect(store.all()).rejects.toThrow(MaestroError);
      await expect(store.create({ title: "blocked by corruption" })).rejects.toThrow(MaestroError);
    });

    it("surfaces duplicate task ids instead of collapsing them on read", async () => {
      const tasksDir = join(tmpDir, ".maestro", "tasks");
      await mkdir(tasksDir, { recursive: true });
      await Bun.write(
        join(tasksDir, "tasks.jsonl"),
        [
          JSON.stringify({
            id: "tsk-abc123",
            title: "First copy",
            type: "task",
            priority: 2,
            status: "open",
            labels: [],
            dependsOn: [],
            createdAt: "2026-04-12T00:00:00.000Z",
            updatedAt: "2026-04-12T00:00:00.000Z",
          }),
          JSON.stringify({
            id: "tsk-abc123",
            title: "Second copy",
            type: "task",
            priority: 2,
            status: "open",
            labels: [],
            dependsOn: [],
            createdAt: "2026-04-12T00:00:01.000Z",
            updatedAt: "2026-04-12T00:00:01.000Z",
          }),
          "",
        ].join("\n"),
      );

      await expect(store.all()).rejects.toThrow(MaestroError);
      await expect(store.update("tsk-abc123", { title: "blocked" })).rejects.toThrow(MaestroError);
    });
  });

  describe("update", () => {
    it("updates title and priority while preserving other fields", async () => {
      const task = await store.create({ title: "Original", priority: 3 });

      const updated = await store.update(task.id, { title: "New title", priority: 1 });

      expect(updated.id).toBe(task.id);
      expect(updated.title).toBe("New title");
      expect(updated.priority).toBe(1);
      expect(updated.type).toBe(task.type);
      expect(updated.createdAt).toBe(task.createdAt);
      expect(updated.updatedAt >= task.updatedAt).toBe(true);
    });

    it("adds and removes labels", async () => {
      const task = await store.create({ title: "L", labels: ["a", "b"] });

      const added = await store.update(task.id, { addLabels: ["c"] });
      expect(added.labels).toEqual(["a", "b", "c"]);

      const removed = await store.update(task.id, { removeLabels: ["a"] });
      expect(removed.labels).toEqual(["b", "c"]);
    });

    it("dedups label additions", async () => {
      const task = await store.create({ title: "L", labels: ["a"] });
      const result = await store.update(task.id, { addLabels: ["a", "b"] });
      expect(result.labels).toEqual(["a", "b"]);
    });

    it("clears parent when empty string passed", async () => {
      const parent = await store.create({ title: "Parent" });
      const child = await store.create({ title: "Child", parentId: parent.id });
      expect(child.parentId).toBe(parent.id);

      const cleared = await store.update(child.id, { parentId: "" });
      expect(cleared.parentId).toBeUndefined();
    });

    it("throws taskNotFound for unknown id", async () => {
      await expect(store.update("tsk-000000", { title: "x" })).rejects.toThrow(MaestroError);
    });

    it("rejects parent cycles inside the locked store transaction", async () => {
      const root = await store.create({ title: "Root" });
      const mid = await store.create({ title: "Mid", parentId: root.id });
      const leaf = await store.create({ title: "Leaf", parentId: mid.id });

      await expect(store.update(root.id, { parentId: leaf.id })).rejects.toThrow(MaestroError);
    });

    it("rejects reopening a claimed task via update", async () => {
      const task = await store.create({ title: "Claimed" });
      await store.claim(task.id, "codex-session-a");

      await expect(store.update(task.id, { status: "open" })).rejects.toThrow(MaestroError);
    });

    it("rejects moving an unclaimed task to in_progress via update", async () => {
      const task = await store.create({ title: "Unclaimed" });

      await expect(store.update(task.id, { status: "in_progress" })).rejects.toThrow(MaestroError);
    });

    it("rejects edits to closed tasks at the store layer", async () => {
      const task = await store.create({ title: "Done" });
      await store.close(task.id, { reason: "shipped" });

      await expect(store.update(task.id, { title: "still mutable" })).rejects.toThrow(MaestroError);
    });
  });

  describe("claim / unclaim", () => {
    it("claims an unowned task", async () => {
      const task = await store.create({ title: "Claim me" });

      const claimed = await store.claim(task.id, "codex-session-a");

      expect(claimed.assignee).toBe("codex-session-a");
      expect(claimed.claimedAt).toBeString();
      expect(claimed.status).toBe("in_progress");
    });

    it("rejects claim by another session without force", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");

      await expect(store.claim(task.id, "codex-session-b")).rejects.toThrow(MaestroError);
    });

    it("force-claims a task from another session", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");

      const claimed = await store.claim(task.id, "codex-session-b", { force: true });

      expect(claimed.assignee).toBe("codex-session-b");
      expect(claimed.status).toBe("in_progress");
    });

    it("unclaims the current owner and reopens in-progress tasks", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");

      const unclaimed = await store.unclaim(task.id, "codex-session-a");

      expect(unclaimed.assignee).toBeUndefined();
      expect(unclaimed.claimedAt).toBeUndefined();
      expect(unclaimed.status).toBe("open");
    });

    it("allows force-unclaim by another session", async () => {
      const task = await store.create({ title: "Claim me" });
      await store.claim(task.id, "codex-session-a");

      const unclaimed = await store.unclaim(task.id, "codex-session-b", { force: true });

      expect(unclaimed.assignee).toBeUndefined();
      expect(unclaimed.status).toBe("open");
    });

    it("resolves competing claim attempts without corrupting storage", async () => {
      const task = await store.create({ title: "Race" });

      const results = await Promise.allSettled([
        store.claim(task.id, "codex-session-a"),
        store.claim(task.id, "codex-session-b"),
      ]);

      const fulfilled = results.filter((result) => result.status === "fulfilled");
      const rejected = results.filter((result) => result.status === "rejected");

      expect(fulfilled).toHaveLength(1);
      expect(rejected).toHaveLength(1);

      const stored = await store.get(task.id);
      expect(stored?.assignee).toBe((fulfilled[0] as PromiseFulfilledResult<{ assignee?: string }>).value.assignee);
    });

    it("normalizes legacy same-owner claims into canonical claimed state", async () => {
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
          dependsOn: [],
          assignee: "codex-legacy",
          createdAt: "2026-04-12T00:00:00.000Z",
          updatedAt: "2026-04-12T00:00:00.000Z",
        })}\n`,
      );

      const claimed = await store.claim("tsk-abc123", "codex-legacy");

      expect(claimed.assignee).toBe("codex-legacy");
      expect(claimed.status).toBe("in_progress");
      expect(claimed.claimedAt).toBeString();
      expect(claimed.updatedAt).not.toBe("2026-04-12T00:00:00.000Z");
    });
  });

  describe("dependency lifecycle", () => {
    it("adds dependencies after creation", async () => {
      const depA = await store.create({ title: "A" });
      const depB = await store.create({ title: "B" });
      const task = await store.create({ title: "Main" });

      const updated = await store.addDependencies(task.id, [depA.id, depB.id]);

      expect(updated.dependsOn).toEqual([depA.id, depB.id]);
    });

    it("returns the existing task without rewriting when dependencies are unchanged", async () => {
      const depA = await store.create({ title: "A" });
      const task = await store.create({ title: "Main", dependsOn: [depA.id] });
      const jsonlPath = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
      const before = await readFile(jsonlPath, "utf8");

      const unchanged = await store.addDependencies(task.id, [depA.id]);
      const after = await readFile(jsonlPath, "utf8");

      expect(unchanged.updatedAt).toBe(task.updatedAt);
      expect(after).toBe(before);
    });

    it("rejects dependency cycles", async () => {
      const taskA = await store.create({ title: "A" });
      const taskB = await store.create({ title: "B", dependsOn: [taskA.id] });

      await expect(store.addDependencies(taskA.id, [taskB.id])).rejects.toThrow(MaestroError);
    });

    it("removes dependencies idempotently", async () => {
      const depA = await store.create({ title: "A" });
      const depB = await store.create({ title: "B" });
      const task = await store.create({ title: "Main", dependsOn: [depA.id, depB.id] });

      const once = await store.removeDependencies(task.id, [depA.id]);
      expect(once.dependsOn).toEqual([depB.id]);

      const twice = await store.removeDependencies(task.id, [depA.id]);
      expect(twice.dependsOn).toEqual([depB.id]);
    });

    it("returns the existing task without rewriting when removeDependencies is a no-op", async () => {
      const depA = await store.create({ title: "A" });
      const task = await store.create({ title: "Main", dependsOn: [depA.id] });
      const jsonlPath = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
      const before = await readFile(jsonlPath, "utf8");

      const unchanged = await store.removeDependencies(task.id, ["tsk-ffffff"]);
      const after = await readFile(jsonlPath, "utf8");

      expect(unchanged.updatedAt).toBe(task.updatedAt);
      expect(after).toBe(before);
    });
  });

  describe("close", () => {
    it("closes a task with a reason", async () => {
      const task = await store.create({ title: "Done" });
      const closed = await store.close(task.id, { reason: "shipped" });
      expect(closed.status).toBe("closed");
      expect(closed.closeReason).toBe("shipped");
    });

    it("closes without a reason", async () => {
      const task = await store.create({ title: "Done" });
      const closed = await store.close(task.id, {});
      expect(closed.status).toBe("closed");
      expect(closed.closeReason).toBeUndefined();
    });

    it("persists closed state across instances", async () => {
      const task = await store.create({ title: "Persist closed" });
      await store.close(task.id, { reason: "r" });

      const fresh = new JsonlTaskStoreAdapter(tmpDir);
      const loaded = await fresh.get(task.id);
      expect(loaded?.status).toBe("closed");
      expect(loaded?.closeReason).toBe("r");
    });

    it("throws taskNotFound for unknown id", async () => {
      await expect(store.close("tsk-000000", {})).rejects.toThrow(MaestroError);
    });

    it("rejects re-closing an already closed task at the store layer", async () => {
      const task = await store.create({ title: "Done" });
      await store.close(task.id, { reason: "first" });

      await expect(store.close(task.id, { reason: "second" })).rejects.toThrow(MaestroError);
    });
  });

  describe("locking", () => {
    it("does not steal a stale-looking lock when the owner pid is still alive", async () => {
      const lockPath = await writeLockFile(tmpDir, { pid: process.pid, createdAt: "2026-04-12T00:00:00.000Z" });
      const stale = new Date(Date.now() - 60_000);
      await utimes(lockPath, stale, stale);

      await expect(store.create({ title: "blocked" })).rejects.toThrow(MaestroError);
      expect((await stat(lockPath)).isFile()).toBe(true);
    }, 8_000);

    it("clears a stale lock when the owner pid is dead", async () => {
      const lockPath = await writeLockFile(tmpDir, { pid: 999_999, createdAt: "2026-04-12T00:00:00.000Z" });
      const stale = new Date(Date.now() - 60_000);
      await utimes(lockPath, stale, stale);

      const created = await store.create({ title: "unblocked" });
      expect(created.title).toBe("unblocked");
      await expect(stat(lockPath)).rejects.toThrow();
    });

    it("waits for a live lock to clear before giving up", async () => {
      const lockPath = await writeLockFile(tmpDir, {
        pid: process.pid,
        createdAt: new Date().toISOString(),
      });

      const releaseLock = (async () => {
        await sleep(1_200);
        await rm(lockPath, { force: true });
      })();

      const created = await store.create({ title: "waited through contention" });
      await releaseLock;

      expect(created.title).toBe("waited through contention");
      await expect(stat(lockPath)).rejects.toThrow();
    }, 8_000);
  });
});

async function writeLockFile(
  baseDir: string,
  metadata: { pid: number; createdAt: string },
): Promise<string> {
  const tasksDir = join(baseDir, ".maestro", "tasks");
  await mkdir(tasksDir, { recursive: true });
  const lockPath = join(tasksDir, ".tasks.lock");
  await Bun.write(lockPath, `${JSON.stringify(metadata)}\n`);
  return lockPath;
}
