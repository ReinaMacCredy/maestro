import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStore } from "@/repo/jsonl-task-store.adapter.js";
import { DuplicateSlugError, TaskNotFoundError } from "@/repo/task-store.port.js";

const FROZEN = new Date("2026-05-15T10:00:00.000Z");

function makeStore(root: string) {
  let n = 0;
  return new JsonlTaskStore({
    repoRoot: root,
    clock: () => FROZEN,
    idFactory: () => `tsk-${++n}`,
  });
}

describe("JsonlTaskStore", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-task-store-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns empty list when no tasks file exists", async () => {
    const store = makeStore(root);
    expect(await store.list()).toEqual([]);
  });

  it("creates, reads back, and updates a task", async () => {
    const store = makeStore(root);
    const created = await store.create({
      slug: "alpha",
      title: "Alpha",
      state: "draft",
    });
    expect(created.id).toBe("tsk-1");
    expect(created.state).toBe("draft");
    const fetched = await store.get("tsk-1");
    expect(fetched?.slug).toBe("alpha");
    const updated = await store.update("tsk-1", { state: "claimed", assignee: "agent-x" });
    expect(updated.state).toBe("claimed");
    expect(updated.assignee).toBe("agent-x");
  });

  it("rejects duplicate slugs", async () => {
    const store = makeStore(root);
    await store.create({ slug: "dup", title: "first", state: "draft" });
    let caught: unknown;
    try {
      await store.create({ slug: "dup", title: "second", state: "draft" });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(DuplicateSlugError);
  });

  it("throws TaskNotFoundError for unknown ids on update", async () => {
    const store = makeStore(root);
    let caught: unknown;
    try {
      await store.update("tsk-missing", { state: "claimed" });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(TaskNotFoundError);
  });

  it("listByState filters correctly", async () => {
    const store = makeStore(root);
    await store.create({ slug: "a", title: "A", state: "draft" });
    await store.create({ slug: "b", title: "B", state: "draft" });
    const c = await store.create({ slug: "c", title: "C", state: "draft" });
    await store.update(c.id, { state: "doing" });
    const drafts = await store.listByState("draft");
    expect(drafts.map((t) => t.slug).sort()).toEqual(["a", "b"]);
    const doing = await store.listByState("doing");
    expect(doing.map((t) => t.slug)).toEqual(["c"]);
  });

  it("listByMissionId filters tasks by their mission_id linkage", async () => {
    const store = makeStore(root);
    await store.create({ slug: "x", title: "X", state: "draft", mission_id: "pln-1" });
    await store.create({ slug: "y", title: "Y", state: "draft", mission_id: "pln-1" });
    await store.create({ slug: "z", title: "Z", state: "draft", mission_id: "pln-2" });
    await store.create({ slug: "no-plan", title: "Solo", state: "draft" });

    const pln1 = await store.listByMissionId("pln-1");
    expect(pln1.map((t) => t.slug).sort()).toEqual(["x", "y"]);

    const pln2 = await store.listByMissionId("pln-2");
    expect(pln2.map((t) => t.slug)).toEqual(["z"]);

    const unknown = await store.listByMissionId("pln-missing");
    expect(unknown).toEqual([]);
  });

  it("serializes concurrent updates without losing writes", async () => {
    const store = makeStore(root);
    const created = await store.create({ slug: "x", title: "X", state: "draft" });
    const ops: Promise<unknown>[] = [];
    for (let i = 0; i < 10; i++) {
      ops.push(store.update(created.id, { title: `X-${i}` }));
    }
    await Promise.all(ops);
    const final = await store.get(created.id);
    expect(final?.title).toMatch(/^X-\d$/);
    const all = await store.list();
    expect(all.length).toBe(1);
  });
});
