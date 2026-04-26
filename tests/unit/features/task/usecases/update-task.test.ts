import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("updateTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-update-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("updates basic fields", async () => {
    const task = await createTask(store, { title: "Original" });
    const { task: updated, autoClaimed } = await updateTask(store, task.id, {
      title: "New",
      priority: 1,
    });

    expect(updated.title).toBe("New");
    expect(updated.priority).toBe(1);
    expect(autoClaimed).toBe(false);
  });

  it("rejects moving an unclaimed task to in_progress", async () => {
    const task = await createTask(store, { title: "Doing" });

    await expect(
      updateTask(store, task.id, { status: "in_progress" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows a claimed task to move to in_progress", async () => {
    const task = await createTask(store, { title: "Doing" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const { task: updated, autoClaimed } = await updateTask(
      store,
      task.id,
      { status: "in_progress" },
      { sessionId: "codex-session-a" },
    );
    expect(updated.status).toBe("in_progress");
    expect(autoClaimed).toBe(false);
  });

  it("rejects moving a claimed task back to pending via update", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });
    await updateTask(store, task.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

    await expect(
      updateTask(store, task.id, { status: "pending" }, { sessionId: "codex-session-a" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows same-owner metadata edits while a task remains claimed and pending", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const { task: updated } = await updateTask(
      store,
      task.id,
      { title: "Retitled while pending" },
      { sessionId: "codex-session-a" },
    );

    expect(updated.title).toBe("Retitled while pending");
    expect(updated.status).toBe("pending");
  });

  it("rejects claimed-task edits without the owning session", async () => {
    const task = await createTask(store, { title: "Claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    await expect(
      updateTask(store, task.id, { title: "Nope" }),
    ).rejects.toThrow(MaestroError);

    await expect(
      updateTask(store, task.id, { title: "Still nope" }, { sessionId: "codex-session-b" }),
    ).rejects.toThrow(MaestroError);
  });

  it("completes a task with a reason", async () => {
    const task = await createTask(store, { title: "Done" });

    const { task: completed } = await updateTask(store, task.id, {
      status: "completed",
      reason: "shipped",
    });

    expect(completed.status).toBe("completed");
    expect(completed.closeReason).toBe("shipped");
  });

  it("rejects completing a task while unresolved blockers remain", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, { title: "Blocked", blockedBy: [blocker.id] });

    await expect(
      updateTask(store, blocked.id, { status: "completed", reason: "skipped ahead" }),
    ).rejects.toThrow(MaestroError);
  });

  it("surfaces blocker error before the unclaimed-in_progress check", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, { title: "Blocked", blockedBy: [blocker.id] });

    await expect(
      updateTask(store, blocked.id, { status: "in_progress" }),
    ).rejects.toThrow(/blocked by unresolved/);
  });

  it("auto-claims an unowned task when transitioning to in_progress with a session", async () => {
    const task = await createTask(store, { title: "To start" });

    const { task: updated, autoClaimed } = await updateTask(
      store,
      task.id,
      { status: "in_progress" },
      { sessionId: "codex-session-a" },
    );

    expect(autoClaimed).toBe(true);
    expect(updated.status).toBe("in_progress");
    expect(updated.assignee).toBe("codex-session-a");
    expect(updated.claimedAt).toBeDefined();
  });

  it("refuses auto-claim when no session can be resolved", async () => {
    const task = await createTask(store, { title: "To start" });

    await expect(
      updateTask(store, task.id, { status: "in_progress" }),
    ).rejects.toThrow(/requires task ownership/);
  });

  it("enforces busy-check on the auto-claim path", async () => {
    const first = await createTask(store, { title: "In flight" });
    const second = await createTask(store, { title: "Next up" });
    await claimTask(store, first.id, { sessionId: "codex-session-a" });
    await updateTask(store, first.id, { status: "in_progress" }, { sessionId: "codex-session-a" });

    await expect(
      updateTask(
        store,
        second.id,
        { status: "in_progress" },
        { sessionId: "codex-session-a" },
      ),
    ).rejects.toThrow(/already owns unresolved/);
  });

  it("reports autoClaimed=false when the caller had already claimed explicitly", async () => {
    const task = await createTask(store, { title: "Pre-claimed" });
    await claimTask(store, task.id, { sessionId: "codex-session-a" });

    const { task: updated, autoClaimed } = await updateTask(
      store,
      task.id,
      { status: "in_progress" },
      { sessionId: "codex-session-a" },
    );

    expect(autoClaimed).toBe(false);
    expect(updated.status).toBe("in_progress");
    expect(updated.assignee).toBe("codex-session-a");
  });

  it("rejects completion reasons without completed status", async () => {
    const task = await createTask(store, { title: "Oops" });

    await expect(
      updateTask(store, task.id, { reason: "not yet" }),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects edits to completed tasks", async () => {
    const task = await createTask(store, { title: "Done" });
    await updateTask(store, task.id, { status: "completed", reason: "shipped" });

    await expect(
      updateTask(store, task.id, { title: "still mutable?" }),
    ).rejects.toThrow(MaestroError);
  });

  it("allows reparenting within a valid tree", async () => {
    const a = await createTask(store, { title: "A" });
    const b = await createTask(store, { title: "B" });
    const leaf = await createTask(store, { title: "leaf", parentId: a.id });

    const { task: moved } = await updateTask(store, leaf.id, { parentId: b.id });
    expect(moved.parentId).toBe(b.id);
  });

  describe("slug lifecycle", () => {
    it("L1: promotes a step to a track only when a slug is supplied", async () => {
      const track = await createTask(store, { title: "Parent track", type: "feature" });
      const step = await createTask(store, { title: "Step", parentId: track.id });

      await expect(
        updateTask(store, step.id, { parentId: "" }),
      ).rejects.toThrow(/Top-level tasks require a slug/);

      const { task: promoted } = await updateTask(store, step.id, {
        parentId: "",
        slug: "implement/promoted-step",
      });
      expect(promoted.parentId).toBeUndefined();
      expect(promoted.slug).toBe("implement/promoted-step");
    });

    it("L2: demoting a track requires --drop-slug when the track has a slug", async () => {
      const trackA = await createTask(store, { title: "A", type: "feature" });
      const trackB = await createTask(store, { title: "B", type: "feature" });

      await expect(
        updateTask(store, trackA.id, { parentId: trackB.id }),
      ).rejects.toThrow(/--drop-slug to confirm/);

      const { task: demoted } = await updateTask(store, trackA.id, {
        parentId: trackB.id,
        dropSlug: true,
      });
      expect(demoted.parentId).toBe(trackB.id);
      expect(demoted.slug).toBeUndefined();
    });

    it("L3: renames a slug in place and rejects collisions", async () => {
      const a = await createTask(store, { title: "Alpha", type: "feature" });
      const b = await createTask(store, { title: "Beta", type: "feature" });

      const { task: renamed } = await updateTask(store, a.id, { slug: "implement/renamed" });
      expect(renamed.slug).toBe("implement/renamed");

      await expect(
        updateTask(store, b.id, { slug: "implement/renamed" }),
      ).rejects.toThrow(/already used by/);
    });

    it("rejects setting a slug on a step task", async () => {
      const track = await createTask(store, { title: "T", type: "feature" });
      const step = await createTask(store, { title: "Step", parentId: track.id });

      await expect(
        updateTask(store, step.id, { slug: "implement/x" }),
      ).rejects.toThrow(/cannot carry a slug/);
    });

    it("L4 regression: deleting a track with steps does not surface a slug error", async () => {
      const track = await createTask(store, { title: "Track", type: "feature" });
      await createTask(store, { title: "Step 1", parentId: track.id });
      await createTask(store, { title: "Step 2", parentId: track.id });

      const deleted = await store.delete(track.id);
      expect(deleted.id).toBe(track.id);
      const remaining = await store.all();
      expect(remaining.every((t) => t.parentId === undefined || t.parentId === track.id)).toBe(true);
    });
  });

  describe("uniqueness", () => {
    it("appends -2 suffix when a derived slug already exists, and gives a clear error after exhausting suffixes", async () => {
      const baseTitle = "Bump deps";
      const created: string[] = [];
      for (let i = 0; i < 9; i++) {
        const t = await createTask(store, { title: baseTitle, type: "chore" });
        if (t.slug) created.push(t.slug);
      }
      expect(created).toEqual([
        "chore/bump-deps",
        "chore/bump-deps-2",
        "chore/bump-deps-3",
        "chore/bump-deps-4",
        "chore/bump-deps-5",
        "chore/bump-deps-6",
        "chore/bump-deps-7",
        "chore/bump-deps-8",
        "chore/bump-deps-9",
      ]);

      await expect(createTask(store, { title: baseTitle, type: "chore" })).rejects.toThrow(
        MaestroError,
      );
    });
  });
});
