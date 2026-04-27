import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("createTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-create-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("creates a pending task with default blocker fields", async () => {
    const task = await createTask(store, { title: "First" });

    expect(task.title).toBe("First");
    expect(task.status).toBe("pending");
    expect(task.priority).toBe(2);
    expect(task.blockedBy).toEqual([]);
    expect(task.blocks).toEqual([]);
  });

  it("rejects empty titles", async () => {
    await expect(createTask(store, { title: "" })).rejects.toThrow(MaestroError);
    await expect(createTask(store, { title: "   " })).rejects.toThrow(MaestroError);
  });

  it("rejects unknown blocker ids", async () => {
    await expect(
      createTask(store, { title: "X", blockedBy: ["tsk-000000"] }),
    ).rejects.toThrow(MaestroError);
  });

  it("creates reciprocal blocker edges when blocked-by is provided", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, {
      title: "Blocked",
      blockedBy: [blocker.id],
    });

    expect(blocked.blockedBy).toEqual([blocker.id]);

    const refreshedBlocker = await store.get(blocker.id);
    expect(refreshedBlocker?.blocks).toEqual([blocked.id]);
  });

  it("accepts an existing parent id", async () => {
    const parent = await createTask(store, { title: "Root" });
    const child = await createTask(store, {
      title: "Leaf",
      parentId: parent.id,
    });

    expect(child.parentId).toBe(parent.id);
  });

  it("trims titles and preserves labels", async () => {
    const task = await createTask(store, {
      title: "  spaced  ",
      labels: ["auth", "urgent"],
    });

    expect(task.title).toBe("spaced");
    expect(task.labels).toEqual(["auth", "urgent"]);
  });

  it("auto-derives a slug for top-level tasks", async () => {
    const task = await createTask(store, { title: "Add login form", type: "feature" });
    expect(task.slug).toBe("implement/add-login-form");
    expect(task.parentId).toBeUndefined();
  });

  it("appends -2..-9 on derived-slug collisions", async () => {
    const a = await createTask(store, { title: "Bump deps", type: "chore" });
    const b = await createTask(store, { title: "Bump deps", type: "chore" });
    const c = await createTask(store, { title: "Bump deps", type: "chore" });
    expect(a.slug).toBe("chore/bump-deps");
    expect(b.slug).toBe("chore/bump-deps-2");
    expect(c.slug).toBe("chore/bump-deps-3");
  });

  it("does not assign a slug to step tasks", async () => {
    const parent = await createTask(store, { title: "Track", type: "feature" });
    const child = await createTask(store, { title: "Step", parentId: parent.id });
    expect(child.slug).toBeUndefined();
    expect(parent.slug).toBe("implement/track");
  });

  it("rejects an explicit slug whose shape is invalid", async () => {
    await expect(createTask(store, { title: "X", slug: "Foo/Bar" })).rejects.toThrow(MaestroError);
  });

  it("surfaces slugCollision when an explicit slug duplicates an existing track", async () => {
    await createTask(store, { title: "First", slug: "implement/foo" });
    await expect(createTask(store, { title: "Second", slug: "implement/foo" })).rejects.toThrow(
      MaestroError,
    );
  });

  it("rejects a slug on a step task at validate time", async () => {
    const parent = await createTask(store, { title: "Track" });
    await expect(
      createTask(store, { title: "Step", parentId: parent.id, slug: "implement/x" }),
    ).rejects.toThrow(MaestroError);
  });
});
