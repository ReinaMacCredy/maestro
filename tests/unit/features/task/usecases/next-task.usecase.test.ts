import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { blockTasks } from "@/features/task/usecases/manage-task-blockers.usecase.js";
import { nextTask } from "@/features/task/usecases/next-task.usecase.js";
import { MaestroError } from "@/shared/errors.js";

describe("nextTask", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-next-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("returns nothing-pending when the queue is empty", async () => {
    const result = await nextTask(store, { sessionId: "codex/s1" });
    expect(result.task).toBeUndefined();
    expect(result.reason).toBe("nothing pending");
  });

  it("returns all-blocked when every pending task has unresolved blockers", async () => {
    const blocker = await createTask(store, { title: "Blocker" });
    const blocked = await createTask(store, { title: "Blocked", blockedBy: [blocker.id] });
    expect(blocked.blockedBy).toContain(blocker.id);

    await claimTask(store, blocker.id, { sessionId: "other/session" });

    const result = await nextTask(store, { sessionId: "codex/s1" });
    expect(result.task).toBeUndefined();
    expect(result.reason).toBe("all blocked");
  });

  it("claims the first ready task and returns it", async () => {
    await createTask(store, { title: "First" });
    await createTask(store, { title: "Second" });

    const result = await nextTask(store, { sessionId: "codex/s1" });
    expect(result.task).toBeDefined();
    expect(result.task?.assignee).toBe("codex/s1");
  });

  it("errors when the session already holds an open task", async () => {
    const held = await createTask(store, { title: "Held" });
    await claimTask(store, held.id, { sessionId: "codex/s1" });
    await createTask(store, { title: "Another" });

    await expect(nextTask(store, { sessionId: "codex/s1" })).rejects.toThrow(MaestroError);
  });

  it("proceeds past a held task when force is set", async () => {
    const held = await createTask(store, { title: "Held" });
    await claimTask(store, held.id, { sessionId: "codex/s1" });

    const nextReady = await createTask(store, { title: "Fresh" });

    const result = await nextTask(store, { sessionId: "codex/s1", force: true });
    expect(result.task?.id).toBe(nextReady.id);
    expect(result.task?.assignee).toBe("codex/s1");
  });

  it("treats a completed task as not held", async () => {
    const done = await createTask(store, { title: "Done" });
    await claimTask(store, done.id, { sessionId: "codex/s1" });
    await updateTask(store, done.id, { status: "completed" }, { sessionId: "codex/s1" });

    await createTask(store, { title: "Next work" });

    const result = await nextTask(store, { sessionId: "codex/s1" });
    expect(result.task).toBeDefined();
    expect(result.task?.assignee).toBe("codex/s1");
  });
});
