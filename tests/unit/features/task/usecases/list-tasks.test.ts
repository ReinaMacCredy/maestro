import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { listTasks } from "@/features/task/usecases/list-tasks.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";

describe("listTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-list-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("returns empty array on empty store", async () => {
    const tasks = await listTasks(store);
    expect(tasks).toEqual([]);
  });

  it("returns all tasks sorted by createdAt", async () => {
    const a = await createTask(store, { title: "A" });
    // Small tick ensures distinct createdAt.
    await new Promise((r) => setTimeout(r, 5));
    const b = await createTask(store, { title: "B" });
    await new Promise((r) => setTimeout(r, 5));
    const c = await createTask(store, { title: "C" });

    const tasks = await listTasks(store);
    expect(tasks.map((t) => t.id)).toEqual([a.id, b.id, c.id]);
  });

  it("filters by status", async () => {
    await createTask(store, { title: "open one" });
    const t2 = await createTask(store, { title: "to close" });
    await store.close(t2.id, { reason: "done" });

    const open = await listTasks(store, { status: "open" });
    const closed = await listTasks(store, { status: "closed" });
    expect(open.length).toBe(1);
    expect(closed.length).toBe(1);
    expect(open[0]?.title).toBe("open one");
  });

  it("filters by priority", async () => {
    await createTask(store, { title: "P0", priority: 0 });
    await createTask(store, { title: "P2", priority: 2 });
    await createTask(store, { title: "P4", priority: 4 });

    const p0 = await listTasks(store, { priority: 0 });
    expect(p0.map((t) => t.title)).toEqual(["P0"]);
  });

  it("filters by type", async () => {
    await createTask(store, { title: "bug one", type: "bug" });
    await createTask(store, { title: "feat one", type: "feature" });

    const bugs = await listTasks(store, { type: "bug" });
    expect(bugs.map((t) => t.title)).toEqual(["bug one"]);
  });

  it("filters by label", async () => {
    await createTask(store, { title: "A", labels: ["auth"] });
    await createTask(store, { title: "B", labels: ["ui"] });
    await createTask(store, { title: "C", labels: ["auth", "ui"] });

    const auth = await listTasks(store, { label: "auth" });
    expect(auth.map((t) => t.title)).toEqual(["A", "C"]);
  });

  it("filters by parent id", async () => {
    const root = await createTask(store, { title: "Root" });
    await createTask(store, { title: "Child 1", parentId: root.id });
    await createTask(store, { title: "Child 2", parentId: root.id });
    await createTask(store, { title: "Unrelated" });

    const children = await listTasks(store, { parentId: root.id });
    expect(children.length).toBe(2);
  });

  it("respects limit", async () => {
    await createTask(store, { title: "A" });
    await createTask(store, { title: "B" });
    await createTask(store, { title: "C" });

    const limited = await listTasks(store, { limit: 2 });
    expect(limited.length).toBe(2);
  });

  it("limit 0 means no limit", async () => {
    await createTask(store, { title: "A" });
    await createTask(store, { title: "B" });
    const all = await listTasks(store, { limit: 0 });
    expect(all.length).toBe(2);
  });
});
