import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("JsonlTaskStoreAdapter.backfillSlug", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-backfill-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("backfills a slugless task synthesized via raw JSONL (legacy data shape)", async () => {
    const path = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
    await Bun.write(
      path,
      JSON.stringify({
        id: "tsk-aaaaaa",
        title: "Legacy track",
        type: "feature",
        priority: 2,
        status: "pending",
        labels: [],
        blocks: [],
        blockedBy: [],
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }) + "\n",
    );

    const updated = await store.backfillSlug("tsk-aaaaaa", "implement/legacy-track");
    expect(updated.slug).toBe("implement/legacy-track");

    const reloaded = await store.get("tsk-aaaaaa");
    expect(reloaded?.slug).toBe("implement/legacy-track");
  });

  it("works on a completed task (slug is display-only metadata)", async () => {
    const path = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
    await Bun.write(
      path,
      JSON.stringify({
        id: "tsk-bbbbbb",
        title: "Old completed work",
        type: "feature",
        priority: 2,
        status: "completed",
        labels: [],
        blocks: [],
        blockedBy: [],
        closeReason: "shipped",
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }) + "\n",
    );

    const updated = await store.backfillSlug("tsk-bbbbbb", "implement/old-completed-work");
    expect(updated.slug).toBe("implement/old-completed-work");
    expect(updated.status).toBe("completed");
  });

  it("works on a claimed task (slug write does not touch ownership)", async () => {
    const path = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
    await Bun.write(
      path,
      JSON.stringify({
        id: "tsk-cccccc",
        title: "Claimed work",
        type: "feature",
        priority: 2,
        status: "in_progress",
        assignee: "operator-x",
        claimedAt: "2026-04-26T00:00:00.000Z",
        labels: [],
        blocks: [],
        blockedBy: [],
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }) + "\n",
    );

    const updated = await store.backfillSlug("tsk-cccccc", "implement/claimed-work");
    expect(updated.slug).toBe("implement/claimed-work");
    expect(updated.assignee).toBe("operator-x");
  });

  it("rejects backfilling a step task", async () => {
    const parent = await createTask(store, { title: "Parent" });
    const step = await createTask(store, { title: "Step", parentId: parent.id });
    await expect(store.backfillSlug(step.id, "implement/step")).rejects.toThrow(
      /cannot carry a slug/,
    );
  });

  it("rejects backfilling when the task already has a slug", async () => {
    const t = await createTask(store, { title: "Existing", slug: "implement/existing" });
    await expect(store.backfillSlug(t.id, "implement/replacement")).rejects.toThrow(
      /already has slug/,
    );
  });

  it("rejects a slug that collides with another track", async () => {
    const path = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
    const lines = [
      JSON.stringify({
        id: "tsk-aaaaaa",
        title: "Slugless legacy",
        type: "feature",
        priority: 2,
        status: "pending",
        labels: [],
        blocks: [],
        blockedBy: [],
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }),
      JSON.stringify({
        id: "tsk-bbbbbb",
        title: "Existing slug",
        type: "feature",
        priority: 2,
        status: "pending",
        labels: [],
        blocks: [],
        blockedBy: [],
        slug: "implement/foo",
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }),
    ];
    await Bun.write(path, lines.join("\n") + "\n");

    await expect(store.backfillSlug("tsk-aaaaaa", "implement/foo")).rejects.toThrow(
      /already used by/,
    );
  });

  it("atomically backfills multiple slugs so rederive swaps can succeed", async () => {
    const first = await createTask(store, { title: "First", slug: "implement/old-second" });
    const second = await createTask(store, { title: "Second", slug: "implement/old-first" });

    const updated = await store.backfillSlugs(
      [
        { id: first.id, slug: "implement/old-first" },
        { id: second.id, slug: "implement/old-second" },
      ],
      { force: true },
    );

    expect(updated.map((task) => task.slug)).toEqual([
      "implement/old-first",
      "implement/old-second",
    ]);
    expect((await store.get(first.id))?.slug).toBe("implement/old-first");
    expect((await store.get(second.id))?.slug).toBe("implement/old-second");
  });

  it("backfilled tasks become resolvable by slug", async () => {
    const path = join(tmpDir, ".maestro", "tasks", "tasks.jsonl");
    await Bun.write(
      path,
      JSON.stringify({
        id: "tsk-dddddd",
        title: "Legacy",
        type: "feature",
        priority: 2,
        status: "pending",
        labels: [],
        blocks: [],
        blockedBy: [],
        createdAt: "2026-04-26T00:00:00.000Z",
        updatedAt: "2026-04-26T00:00:00.000Z",
      }) + "\n",
    );

    await store.backfillSlug("tsk-dddddd", "implement/legacy");
    const updateResult = await updateTask(store, "tsk-dddddd", { title: "Renamed" });
    expect(updateResult.task.title).toBe("Renamed");
    expect(updateResult.task.slug).toBe("implement/legacy");
  });
});
