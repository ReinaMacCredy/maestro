import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { listTasks } from "@/features/task/usecases/list-tasks.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";

describe("listTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-list-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("returns all tasks sorted by createdAt", async () => {
    const a = await createTask(store, { title: "A" });
    await new Promise((r) => setTimeout(r, 5));
    const b = await createTask(store, { title: "B" });
    await new Promise((r) => setTimeout(r, 5));
    const c = await createTask(store, { title: "C" });

    const tasks = await listTasks(store);
    expect(tasks.map((t) => t.id)).toEqual([a.id, b.id, c.id]);
  });

  it("filters by completed status", async () => {
    await createTask(store, { title: "pending one" });
    const done = await createTask(store, { title: "done one" });
    await updateTask(store, done.id, { status: "completed", reason: "done" });

    const completed = await listTasks(store, { status: "completed" });
    expect(completed.map((t) => t.title)).toEqual(["done one"]);
  });

  it("filters by priority, type, label, parent, and assignee", async () => {
    const parent = await createTask(store, { title: "Root" });
    const owned = await createTask(store, {
      title: "Bug",
      type: "bug",
      priority: 0,
      labels: ["auth"],
      parentId: parent.id,
    });
    await store.claim(owned.id, "alice");
    await createTask(store, { title: "Other", type: "feature", labels: ["ui"] });

    expect((await listTasks(store, { priority: 0 })).map((t) => t.title)).toEqual(["Bug"]);
    expect((await listTasks(store, { type: "bug" })).map((t) => t.title)).toEqual(["Bug"]);
    expect((await listTasks(store, { label: "auth" })).map((t) => t.title)).toEqual(["Bug"]);
    expect((await listTasks(store, { parentId: parent.id })).map((t) => t.title)).toEqual(["Bug"]);
    expect((await listTasks(store, { assignee: "alice" })).map((t) => t.title)).toEqual(["Bug"]);
  });

  it("respects limit", async () => {
    await createTask(store, { title: "A" });
    await createTask(store, { title: "B" });
    await createTask(store, { title: "C" });

    const limited = await listTasks(store, { limit: 2 });
    expect(limited.length).toBe(2);
  });
});
