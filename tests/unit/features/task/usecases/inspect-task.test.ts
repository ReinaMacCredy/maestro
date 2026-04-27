import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsTaskContinuationHistoryStoreAdapter } from "@/features/task/adapters/fs-task-continuation-history-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "@/features/task/adapters/fs-task-continuation-store.adapter.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { formatTaskShowView } from "@/features/task/commands/task-command-formatters.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { inspectTask } from "@/features/task/usecases/inspect-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";

describe("inspectTask", () => {
  let tmpDir: string;
  let taskStore: JsonlTaskStoreAdapter;
  let continuationStore: FsTaskContinuationStoreAdapter;
  let continuationHistory: FsTaskContinuationHistoryStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-inspect-"));
    taskStore = new JsonlTaskStoreAdapter(tmpDir);
    continuationStore = new FsTaskContinuationStoreAdapter(tmpDir);
    continuationHistory = new FsTaskContinuationHistoryStoreAdapter(tmpDir);
  });

  it("returns one task detail read model for blockers, continuation, timeline, and open handoffs", async () => {
    const completedBlocker = await createTask(taskStore, { title: "Done blocker" });
    await updateTask(taskStore, completedBlocker.id, { status: "completed", reason: "done" });
    const activeBlocker = await createTask(taskStore, { title: "Active blocker" });
    const task = await createTask(taskStore, {
      title: "Inspect me",
      blockedBy: [completedBlocker.id, activeBlocker.id],
    });
    await continuationStore.upsertActive({
      taskId: task.id,
      status: "pending",
      lastActiveAt: "2026-04-23T02:00:00.000Z",
      currentState: "Waiting on one blocker",
      nextAction: "Resume implementation",
      keyDecisions: ["keep CLI stable"],
    });
    await continuationHistory.append(task.id, {
      kind: "snapshot",
      at: "2026-04-23T02:00:00.000Z",
      summary: "Captured inspection state",
      currentState: "Waiting on one blocker",
    });

    const view = await inspectTask({
      taskStore,
      continuationStore,
      continuationHistory,
      listOpenHandoffIds: async () => ["handoff-1"],
    }, task.id);

    expect(view.task.id).toBe(task.id);
    expect(view.activeBlockerIds).toEqual([activeBlocker.id]);
    expect(view.openHandoffs).toEqual(["handoff-1"]);
    expect(view.continuation?.currentState).toBe("Waiting on one blocker");
    expect(view.recentEvents.map((event) => event.kind)).toEqual(["snapshot"]);

    const rendered = formatTaskShowView(view);
    expect(rendered).toContain(`  Blocked by: ${activeBlocker.id}`);
    expect(rendered.some((line) => line.includes(completedBlocker.id))).toBe(false);
    expect(rendered).toContain("  Open handoffs: handoff-1");
  });

  it("includes step tasks when inspecting a top-level track", async () => {
    const track = await createTask(taskStore, { title: "Track" });
    const step = await createTask(taskStore, { title: "Step", parentId: track.id });

    const view = await inspectTask({
      taskStore,
      continuationStore,
      continuationHistory,
    }, track.id);

    expect(view.steps?.map((task) => task.id)).toEqual([step.id]);
  });
});
