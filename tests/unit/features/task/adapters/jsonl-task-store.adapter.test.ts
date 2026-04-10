import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
      const task = await store.create({
        title: "Custom",
        priority: 0,
        type: "bug",
        labels: ["urgent", "auth"],
        dependsOn: ["tsk-000001"],
      });

      expect(task.priority).toBe(0);
      expect(task.type).toBe("bug");
      expect(task.labels).toEqual(["urgent", "auth"]);
      expect(task.dependsOn).toEqual(["tsk-000001"]);
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
  });
});
