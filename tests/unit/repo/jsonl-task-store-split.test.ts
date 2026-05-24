import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStore } from "@/repo/jsonl-task-store.adapter.js";
import {
  DuplicateSlugError,
  DuplicateTaskIdError,
  InvalidTaskIdError,
  TaskNotFoundError,
} from "@/repo/task-store.port.js";

const FROZEN = new Date("2026-05-15T10:00:00.000Z");

function makeStore(root: string) {
  let n = 0;
  return new JsonlTaskStore({
    repoRoot: root,
    clock: () => FROZEN,
    idFactory: () => `tsk-auto-${++n}`,
  });
}

describe("JsonlTaskStore.splitTask", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-task-store-split-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("applies parentPatch and stamps parent_id on every child (happy path)", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "parent",
      title: "Parent",
      state: "draft",
    });

    const result = await store.splitTask({
      parentId: parent.id,
      parentPatch: { state: "blocked" },
      childInputs: [
        { slug: "c1", title: "child 1", state: "ready" },
        { slug: "c2", title: "child 2", state: "ready" },
      ],
    });

    expect(result.parent.state).toBe("blocked");
    expect(result.children.length).toBe(2);
    expect(result.children[0]?.parent_id).toBe(parent.id);
    expect(result.children[1]?.parent_id).toBe(parent.id);

    const persistedParent = await store.get(parent.id);
    expect(persistedParent?.state).toBe("blocked");
    const persistedC1 = await store.get(result.children[0]!.id);
    expect(persistedC1?.parent_id).toBe(parent.id);
    const persistedC2 = await store.get(result.children[1]!.id);
    expect(persistedC2?.parent_id).toBe(parent.id);
  });

  it("throws TaskNotFoundError and rolls back when parentId is missing", async () => {
    const store = makeStore(root);
    const other = await store.create({
      slug: "other",
      title: "Other",
      state: "draft",
    });

    let caught: unknown;
    try {
      await store.splitTask({
        parentId: "tsk-nope-nope",
        parentPatch: { state: "blocked" },
        childInputs: [{ slug: "c1", title: "child 1", state: "ready" }],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(TaskNotFoundError);

    expect(await store.get("tsk-nope-nope")).toBeUndefined();
    const stillThere = await store.get(other.id);
    expect(stillThere?.slug).toBe("other");
    const all = await store.list();
    expect(all.map((t) => t.slug).sort()).toEqual(["other"]);
  });

  it("rolls back when a child slug duplicates an existing task", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "parent",
      title: "Parent",
      state: "draft",
    });
    await store.create({ slug: "taken", title: "Taken", state: "draft" });

    let caught: unknown;
    try {
      await store.splitTask({
        parentId: parent.id,
        parentPatch: { state: "blocked" },
        childInputs: [{ slug: "taken", title: "Dup", state: "ready" }],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(DuplicateSlugError);

    const persistedParent = await store.get(parent.id);
    expect(persistedParent?.state).toBe("draft");
    const all = await store.list();
    expect(all.map((t) => t.slug).sort()).toEqual(["parent", "taken"]);
  });

  it("rolls back when two children share a slug in the same batch", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "parent",
      title: "Parent",
      state: "draft",
    });

    let caught: unknown;
    try {
      await store.splitTask({
        parentId: parent.id,
        parentPatch: { state: "blocked" },
        childInputs: [
          { slug: "dup", title: "First", state: "ready" },
          { slug: "dup", title: "Second", state: "ready" },
        ],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(DuplicateSlugError);

    const persistedParent = await store.get(parent.id);
    expect(persistedParent?.state).toBe("draft");
    const all = await store.list();
    expect(all.map((t) => t.slug)).toEqual(["parent"]);
  });

  it("honors explicit child ids", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "parent",
      title: "Parent",
      state: "draft",
    });

    const result = await store.splitTask({
      parentId: parent.id,
      parentPatch: { state: "blocked" },
      childInputs: [
        { id: "tsk-fixed-one", slug: "c1", title: "child 1", state: "ready" },
      ],
    });

    expect(result.children[0]?.id).toBe("tsk-fixed-one");
    const persisted = await store.get("tsk-fixed-one");
    expect(persisted?.id).toBe("tsk-fixed-one");
    expect(persisted?.parent_id).toBe(parent.id);
  });

  it("rejects empty childInputs", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "p",
      title: "Parent",
      state: "draft",
    });
    let caught: unknown;
    try {
      await store.splitTask({
        parentId: parent.id,
        parentPatch: { state: "blocked" },
        childInputs: [],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(Error);
    expect((caught as Error).message).toMatch(/at least one child/);
    // Verify parent was not mutated (no updated_at bump, no state change)
    const fetched = await store.get(parent.id);
    expect(fetched?.state).toBe("draft");
    expect(fetched?.updated_at).toBe(parent.updated_at);
  });

  it("rejects a child input with invalid id format", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "p",
      title: "Parent",
      state: "draft",
    });
    let caught: unknown;
    try {
      await store.splitTask({
        parentId: parent.id,
        parentPatch: {},
        childInputs: [
          { id: "bad-id", slug: "c1", title: "Child 1", state: "draft" },
        ],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(InvalidTaskIdError);
    // Rollback: no child landed; parent unchanged.
    expect(await store.get(parent.id)).toEqual(parent);
    const all = await store.list();
    expect(all.length).toBe(1);
  });

  it("rejects two children sharing the same provided id", async () => {
    const store = makeStore(root);
    const parent = await store.create({
      slug: "p",
      title: "Parent",
      state: "draft",
    });
    let caught: unknown;
    try {
      await store.splitTask({
        parentId: parent.id,
        parentPatch: {},
        childInputs: [
          { id: "tsk-dup-x", slug: "c1", title: "C1", state: "draft" },
          { id: "tsk-dup-x", slug: "c2", title: "C2", state: "draft" },
        ],
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(DuplicateTaskIdError);
    // Rollback: no children landed.
    const all = await store.list();
    expect(all.length).toBe(1);
  });

  it("preserves parent's own parent_id through splitTask (grandparent scenario)", async () => {
    const store = makeStore(root);
    // Seed a 3-level hierarchy: grandparent already exists implicitly via parent_id reference.
    const parent = await store.create({
      slug: "p",
      title: "Parent",
      state: "draft",
      parent_id: "tsk-grand-aaa",
    });
    expect(parent.parent_id).toBe("tsk-grand-aaa");

    const { parent: updatedParent, children } = await store.splitTask({
      parentId: parent.id,
      parentPatch: { state: "blocked" },
      childInputs: [
        { slug: "c1", title: "C1", state: "draft" },
      ],
    });

    expect(updatedParent.state).toBe("blocked");
    expect(updatedParent.parent_id).toBe("tsk-grand-aaa"); // <- the key assertion
    expect(children[0]?.parent_id).toBe(parent.id);

    // Re-read from disk to confirm persistence.
    const refetched = await store.get(parent.id);
    expect(refetched?.parent_id).toBe("tsk-grand-aaa");
  });
});
