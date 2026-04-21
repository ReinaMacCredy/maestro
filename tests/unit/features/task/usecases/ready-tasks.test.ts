import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsCandidateStoreAdapter } from "@/features/task/adapters/fs-candidate-store.adapter.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import type { CandidateStorePort } from "@/features/task/ports/candidate-store.port.js";
import { captureTaskCandidate } from "@/features/task/usecases/capture-task-candidate.usecase.js";
import { claimTask } from "@/features/task/usecases/claim-task.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { readyTaskPage, readyTasks } from "@/features/task/usecases/ready-tasks.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";

describe("readyTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-ready-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("returns only pending tasks", async () => {
    const pending = await createTask(store, { title: "keep pending" });
    const working = await createTask(store, { title: "working" });
    await claimTask(store, working.id, { sessionId: "alice" });
    await updateTask(store, working.id, { status: "in_progress" }, { sessionId: "alice" });
    const done = await createTask(store, { title: "done" });
    await updateTask(store, done.id, { status: "completed", reason: "done" });

    const result = await readyTasks(store);
    expect(result.map((t) => t.id)).toEqual([pending.id]);
  });

  it("excludes tasks with unresolved blockers and unblocks them when blockers complete", async () => {
    const blocker = await createTask(store, { title: "blocker" });
    const blocked = await createTask(store, {
      title: "blocked",
      blockedBy: [blocker.id],
    });

    expect((await readyTasks(store)).map((t) => t.id)).toEqual([blocker.id]);

    await updateTask(store, blocker.id, { status: "completed", reason: "done" });
    expect((await readyTasks(store)).map((t) => t.id)).toEqual([blocked.id]);
  });

  it("keeps ownership separate from readiness filters", async () => {
    const mine = await createTask(store, { title: "mine" });
    const unassigned = await createTask(store, { title: "unassigned" });
    await claimTask(store, mine.id, { sessionId: "alice" });

    expect((await readyTasks(store)).map((t) => t.title)).toEqual(["unassigned"]);
    expect((await readyTasks(store, { assignee: "alice" })).map((t) => t.title)).toEqual(["mine"]);
    expect((await readyTasks(store, { unassigned: true })).map((t) => t.title)).toEqual(["unassigned"]);
  });

  it("filters by label, priority, and type", async () => {
    await createTask(store, { title: "auth bug", labels: ["auth"], priority: 0, type: "bug" });
    await createTask(store, { title: "ui feature", labels: ["ui"], priority: 2, type: "feature" });

    expect((await readyTasks(store, { label: "auth" })).map((t) => t.title)).toEqual(["auth bug"]);
    expect((await readyTasks(store, { priority: 0 })).map((t) => t.title)).toEqual(["auth bug"]);
    expect((await readyTasks(store, { type: "feature" })).map((t) => t.title)).toEqual(["ui feature"]);
  });

  it("sorts high priority work before the rest and respects limits", async () => {
    const p2a = await createTask(store, { title: "P2 first", priority: 2 });
    await new Promise((r) => setTimeout(r, 5));
    const p0 = await createTask(store, { title: "P0", priority: 0 });
    await new Promise((r) => setTimeout(r, 5));
    const p4 = await createTask(store, { title: "P4", priority: 4 });
    await new Promise((r) => setTimeout(r, 5));
    const p1 = await createTask(store, { title: "P1", priority: 1 });

    const result = await readyTasks(store, { limit: 3 });
    expect(result.map((t) => t.id)).toEqual([p0.id, p1.id, p2a.id]);
    expect(result).not.toContainEqual(expect.objectContaining({ id: p4.id }));
  });

  it("returns page metadata before limit slicing", async () => {
    await createTask(store, { title: "P2 first", priority: 2 });
    await new Promise((r) => setTimeout(r, 5));
    const p0 = await createTask(store, { title: "P0", priority: 0 });
    await new Promise((r) => setTimeout(r, 5));
    await createTask(store, { title: "P4", priority: 4 });

    const page = await readyTaskPage(store, { limit: 1 });
    expect(page.totalReady).toBe(3);
    expect(page.items.map((t) => t.id)).toEqual([p0.id]);
  });

  it("treats limit 0 as unbounded in ready task pages", async () => {
    const first = await createTask(store, { title: "first" });
    await new Promise((r) => setTimeout(r, 5));
    const second = await createTask(store, { title: "second" });

    const page = await readyTaskPage(store, { limit: 0 });
    expect(page.totalReady).toBe(2);
    expect(page.items.map((t) => t.id)).toEqual([first.id, second.id]);
  });

  it("applies filters to ready task pages", async () => {
    await createTask(store, { title: "auth bug", labels: ["auth"], priority: 0, type: "bug" });
    await createTask(store, { title: "ui feature", labels: ["ui"], priority: 2, type: "feature" });
    const mine = await createTask(store, { title: "mine" });
    await claimTask(store, mine.id, { sessionId: "alice" });

    expect((await readyTaskPage(store, { label: "auth" })).items.map((t) => t.title)).toEqual(["auth bug"]);
    expect((await readyTaskPage(store, { priority: 0 })).items.map((t) => t.title)).toEqual(["auth bug"]);
    expect((await readyTaskPage(store, { type: "feature" })).items.map((t) => t.title)).toEqual(["ui feature"]);
    expect((await readyTaskPage(store, { assignee: "alice" })).items.map((t) => t.title)).toEqual(["mine"]);
    expect((await readyTaskPage(store, { unassigned: true })).items.map((t) => t.title)).toEqual([
      "auth bug",
      "ui feature",
    ]);
  });

  it("skips candidate reads when nothing is ready", async () => {
    const blocker = await createTask(store, { title: "blocker" });
    await createTask(store, { title: "blocked", blockedBy: [blocker.id] });

    const candidateStore: CandidateStorePort = {
      create: async () => {
        throw new Error("candidate creation should not run in readyTasks");
      },
      all: async () => {
        throw new Error("candidate reads should be skipped when nothing is ready");
      },
      delete: async () => {
        throw new Error("candidate delete should not run in readyTasks");
      },
    };

    await claimTask(store, blocker.id, { sessionId: "alice" });
    await updateTask(store, blocker.id, { status: "in_progress" }, { sessionId: "alice" });
    const result = await readyTasks(store, {}, new Date(), candidateStore);
    expect(result).toEqual([]);
  });

  it("attaches hints from past completed tasks", async () => {
    const candidateStore = new FsCandidateStoreAdapter(tmpDir);
    const past = await createTask(store, { title: "Implement argon2 password hashing" });
    const { task: closed } = await updateTask(store, past.id, {
      status: "completed",
      reason: "argon2 compare was backwards",
    });
    await captureTaskCandidate(candidateStore, closed);

    await createTask(store, { title: "JWT password middleware" });

    const result = await readyTasks(store, {}, new Date(), candidateStore);
    expect(result[0]?.hints.length).toBeGreaterThanOrEqual(1);
    expect(result[0]?.hints[0]?.sourceTaskId).toBe(past.id);
  });
});
