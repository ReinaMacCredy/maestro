import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { planTasks } from "@/features/task/usecases/plan-tasks.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("planTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-plan-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("creates a batch of tasks atomically with name-slot blockers", async () => {
    const result = await planTasks(store, {
      tasks: [
        { name: "first", title: "First", priority: 1 },
        { name: "second", title: "Second", blockedBy: ["first"] },
        { name: "third", title: "Third", blockedBy: ["second"] },
      ],
    });

    expect(result.created).toHaveLength(3);
    expect(result.created.map((t) => t.name)).toEqual(["first", "second", "third"]);
    expect(result.created.every((t) => t.status === "pending")).toBe(true);

    const all = await store.all();
    expect(all).toHaveLength(3);

    const byName = new Map(result.created.map((t) => [t.name!, t.id]));
    const second = await store.get(byName.get("second")!);
    expect(second?.blockedBy).toEqual([byName.get("first")!]);
    const first = await store.get(byName.get("first")!);
    expect(first?.blocks).toContain(byName.get("second")!);
  });

  it("accepts real task ids mixed with batch-local names in blockedBy", async () => {
    const existing = await createTask(store, { title: "Existing blocker" });

    const result = await planTasks(store, {
      tasks: [
        { name: "a", title: "A", blockedBy: [existing.id] },
        { name: "b", title: "B", blockedBy: ["a", existing.id] },
      ],
    });

    const byName = new Map(result.created.map((t) => [t.name!, t.id]));
    const b = await store.get(byName.get("b")!);
    expect(b?.blockedBy).toEqual([byName.get("a")!, existing.id]);
  });

  it("rejects a batch that forms a cycle between two new tasks", async () => {
    await expect(
      planTasks(store, {
        tasks: [
          { name: "x", title: "x", blockedBy: ["y"] },
          { name: "y", title: "y", blockedBy: ["x"] },
        ],
      }),
    ).rejects.toThrow(/blocker cycle/);

    expect(await store.all()).toHaveLength(0);
  });

  it("rejects self-blocking task", async () => {
    await expect(
      planTasks(store, {
        tasks: [{ name: "solo", title: "solo", blockedBy: ["solo"] }],
      }),
    ).rejects.toThrow(/cannot block itself/);
  });

  it("rejects a parent cycle across batch members", async () => {
    await expect(
      planTasks(store, {
        tasks: [
          { name: "p", title: "p", parent: "c" },
          { name: "c", title: "c", parent: "p" },
        ],
      }),
    ).rejects.toThrow(/[Cc]yclic parent/);
  });

  it("rejects duplicate batch-local names", async () => {
    await expect(
      planTasks(store, {
        tasks: [
          { name: "dup", title: "A" },
          { name: "dup", title: "B" },
        ],
      }),
    ).rejects.toThrow(/Duplicate name 'dup'/);
  });

  it("rejects a name slot that looks like a real task id", async () => {
    await expect(
      planTasks(store, {
        tasks: [{ name: "tsk-abc123", title: "fake" }],
      }),
    ).rejects.toThrow(/reserved task id pattern/);
  });

  it("rejects unknown name references", async () => {
    await expect(
      planTasks(store, {
        tasks: [{ name: "only", title: "only", blockedBy: ["missing"] }],
      }),
    ).rejects.toThrow(/Unknown blockedBy reference 'missing'/);
  });

  it("rejects unknown real-id references", async () => {
    await expect(
      planTasks(store, {
        tasks: [{ name: "only", title: "only", blockedBy: ["tsk-000000"] }],
      }),
    ).rejects.toThrow(/references unknown blocker/);
  });

  it("collects multiple per-task validation issues into one error", async () => {
    await expect(
      planTasks(store, {
        tasks: [
          { name: "ok", title: "ok" },
          { name: "bad-title", title: "" },
          { name: "bad-type", title: "t", type: "invalid" as unknown as "task" },
        ],
      }),
    ).rejects.toThrow(/Plan validation failed with 2 issues/);
  });

  it("rejects a malformed tasks array", async () => {
    await expect(
      planTasks(store, { tasks: [] }),
    ).rejects.toThrow(/non-empty array/);
  });

  it("rejects oversized batches", async () => {
    const over: { title: string }[] = [];
    for (let i = 0; i < 6; i++) over.push({ title: `t${i}` });
    await expect(
      planTasks(store, { tasks: over }, { maxBatchSize: 5 }),
    ).rejects.toThrow(/max 5 per batch/);
  });

  it("writes nothing when validation fails (atomicity)", async () => {
    await expect(
      planTasks(store, {
        tasks: [
          { name: "good", title: "good" },
          { name: "bad", title: "", blockedBy: ["good"] },
        ],
      }),
    ).rejects.toThrow(MaestroError);

    expect(await store.all()).toHaveLength(0);
  });

  it("carries batchId through to the result when provided", async () => {
    const result = await planTasks(store, {
      batchId: "batch-123",
      tasks: [{ name: "a", title: "A" }],
    });
    expect(result.batchId).toBe("batch-123");
  });

  it("resolves parent by name slot", async () => {
    const result = await planTasks(store, {
      tasks: [
        { name: "root", title: "root" },
        { name: "leaf", title: "leaf", parent: "root" },
      ],
    });
    const byName = new Map(result.created.map((t) => [t.name!, t.id]));
    const leaf = await store.get(byName.get("leaf")!);
    expect(leaf?.parentId).toBe(byName.get("root")!);
  });

  describe("idempotency via batchId", () => {
    it("returns cached receipt on repeat submission with same batchId", async () => {
      const input = {
        batchId: "idem-1",
        tasks: [
          { name: "a", title: "A" },
          { name: "b", title: "B", blockedBy: ["a"] },
        ],
      };

      const first = await planTasks(store, input);
      const second = await planTasks(store, input);

      expect(second.batchId).toBe("idem-1");
      expect(second.created.map((t) => t.id)).toEqual(first.created.map((t) => t.id));
      expect(await store.all()).toHaveLength(2);
    });

    it("persists a receipt only when batchId is provided", async () => {
      await planTasks(store, { tasks: [{ name: "x", title: "X" }] });
      expect(await store.findBatchReceipt("nonexistent")).toBeUndefined();

      await planTasks(store, {
        batchId: "has-id",
        tasks: [{ name: "y", title: "Y" }],
      });
      const receipt = await store.findBatchReceipt("has-id");
      expect(receipt?.created).toHaveLength(1);
    });

    it("hard-fails replay when cached ids are missing from the store", async () => {
      await planTasks(store, {
        batchId: "drift-1",
        tasks: [{ name: "a", title: "A" }],
      });

      const tasksPath = `${tmpDir}/.maestro/tasks/tasks.jsonl`;
      await Bun.write(tasksPath, "");

      await expect(
        planTasks(store, {
          batchId: "drift-1",
          tasks: [{ name: "a", title: "A" }],
        }),
      ).rejects.toThrow(/stale receipt.*missing from store/);
    });

    it("rejects invalid batchId shapes at the adapter boundary", async () => {
      await expect(
        planTasks(store, {
          batchId: "../hack",
          tasks: [{ name: "a", title: "A" }],
        }),
      ).rejects.toThrow(/Invalid batchId/);
    });
  });

  describe("mandatory slug at plan conversion (PC1-PC9)", () => {
    it("PC1: rejects an explicit slug whose shape is invalid", async () => {
      await expect(
        planTasks(store, {
          tasks: [{ name: "a", title: "A", slug: "Foo/Bar" }],
        }),
      ).rejects.toThrow(/Plan validation failed/);
      expect(await store.all()).toHaveLength(0);
    });

    it("PC3+PC7: derives a slug for top-level entries that omit it", async () => {
      const result = await planTasks(store, {
        tasks: [
          { name: "first", title: "Add login form", type: "feature" },
          { name: "second", title: "Fix race in writer", type: "bug", parent: "first" },
        ],
      });
      const created = result.created;
      const firstId = created.find((t) => t.name === "first")!.id;
      const first = await store.get(firstId);
      expect(first?.slug).toBe("implement/add-login-form");

      const secondId = created.find((t) => t.name === "second")!.id;
      const second = await store.get(secondId);
      expect(second?.slug).toBeUndefined();
      expect(second?.parentId).toBe(firstId);
    });

    it("PC4: rejects two batch entries that derive to the same slug atomically", async () => {
      await expect(
        planTasks(store, {
          tasks: [
            { name: "a", title: "Bump deps", type: "chore" },
            { name: "b", title: "Bump deps", type: "chore", slug: "chore/bump-deps" },
          ],
        }),
      ).rejects.toThrow(/Plan validation failed/);
      expect(await store.all()).toHaveLength(0);
    });

    it("PC5+PC9: rejects a batch slug that collides with an on-disk top-level slug", async () => {
      await createTask(store, { title: "Existing", type: "feature", slug: "implement/existing" });

      await expect(
        planTasks(store, {
          tasks: [{ name: "a", title: "A", slug: "implement/existing" }],
        }),
      ).rejects.toThrow(/already used by an existing/);
      expect(await store.all()).toHaveLength(1);
    });

    it("PC6: rejects an entry that has both 'slug' and 'parent'", async () => {
      await expect(
        planTasks(store, {
          tasks: [
            { name: "root", title: "Root", type: "feature" },
            { name: "child", title: "Child", parent: "root", slug: "implement/child" },
          ],
        }),
      ).rejects.toThrow(/forbidden on step entries/);
      expect(await store.all()).toHaveLength(0);
    });

    it("PC8: a slug from one entry can be used as a cross-reference by another", async () => {
      const result = await planTasks(store, {
        tasks: [
          { title: "Root", type: "feature", slug: "implement/foo" },
          { title: "Child", parent: "implement/foo" },
        ],
      });
      expect(result.created).toHaveLength(2);
      const child = await store.get(result.created[1]!.id);
      const root = await store.get(result.created[0]!.id);
      expect(child?.parentId).toBe(root!.id);
    });

    it("auto-derives suffixes when multiple top-level entries share a base", async () => {
      const result = await planTasks(store, {
        tasks: [
          { name: "a", title: "Bump deps", type: "chore" },
          { name: "b", title: "Bump deps", type: "chore" },
          { name: "c", title: "Bump deps", type: "chore" },
        ],
      });
      const slugs = await Promise.all(
        result.created.map(async (t) => (await store.get(t.id))?.slug),
      );
      expect(slugs).toEqual([
        "chore/bump-deps",
        "chore/bump-deps-2",
        "chore/bump-deps-3",
      ]);
    });
  });
});
