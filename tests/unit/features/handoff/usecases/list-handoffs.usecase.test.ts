import { describe, expect, it } from "bun:test";
import { listHandoffs } from "@/features/handoff";
import type { LegacyTask as Task } from "@/shared/domain/legacy-task";
import { makeHandoffRecord, mockHandoffStore } from "../../../../helpers/mocks.js";

function makeTask(id: string, status: Task["status"]): Task {
  return {
    id,
    title: `Task ${id}`,
    type: "task",
    priority: 2,
    status,
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-04-23T00:00:00.000Z",
    updatedAt: "2026-04-23T00:00:00.000Z",
  };
}

describe("listHandoffs", () => {
  const open1 = makeHandoffRecord({ id: "alpha-fox-1", createdAt: "2026-04-20T00:00:00.000Z" });
  const open2 = makeHandoffRecord({ id: "beta-bear-2", createdAt: "2026-04-21T00:00:00.000Z" });
  const consumed = makeHandoffRecord({
    id: "gamma-owl-3",
    createdAt: "2026-04-22T00:00:00.000Z",
    consumedAt: "2026-04-22T01:00:00.000Z",
  });

  it("returns every record, newest first, by default", async () => {
    const store = mockHandoffStore([open1, open2, consumed]);
    const result = await listHandoffs(store);
    expect(result.map((r) => r.id)).toEqual(["gamma-owl-3", "beta-bear-2", "alpha-fox-1"]);
  });

  it("filters to open packets when openOnly is set", async () => {
    const store = mockHandoffStore([open1, open2, consumed]);
    const result = await listHandoffs(store, { openOnly: true });
    expect(result.map((r) => r.id)).toEqual(["beta-bear-2", "alpha-fox-1"]);
  });

  it("treats completed packets as closed even when consumedAt is absent", async () => {
    const completed = makeHandoffRecord({
      id: "delta-lark-4",
      createdAt: "2026-04-23T00:00:00.000Z",
      status: "completed",
    });
    const store = mockHandoffStore([open1, completed]);
    const result = await listHandoffs(store, { openOnly: true });
    expect(result.map((r) => r.id)).toEqual(["alpha-fox-1"]);
  });

  it("reconciles launched task-linked packets whose linked task already completed", async () => {
    const stale = makeHandoffRecord({
      id: "stale-heron-5",
      createdAt: "2026-04-23T00:00:00.000Z",
      refs: { taskId: "tsk-done" },
      status: "launched",
    });
    const store = mockHandoffStore([open1, stale]);
    const taskStore = {
      async get(id: string) {
        return id === "tsk-done" ? makeTask(id, "completed") : undefined;
      },
    };

    const result = await listHandoffs(store, {
      openOnly: true,
      taskStore,
      currentProjectRoot: "/src",
    });
    expect(result.map((r) => r.id)).toEqual(["alpha-fox-1"]);

    const reconciled = await store.get("stale-heron-5");
    expect(reconciled?.status).toBe("completed");
    expect(reconciled?.consumedAt).toBeUndefined();
  });

  it("does not reconcile foreign-project packets that reuse a completed local task id", async () => {
    const foreign = makeHandoffRecord({
      id: "foreign-heron-6",
      createdAt: "2026-04-23T02:00:00.000Z",
      refs: { taskId: "tsk-done" },
      status: "launched",
      sourceDir: "/repo/other",
    });
    const store = mockHandoffStore([foreign]);

    const result = await listHandoffs(store, {
      openOnly: true,
      taskStore: {
        async get(id: string) {
          return id === "tsk-done" ? makeTask(id, "completed") : undefined;
        },
      },
      currentProjectRoot: "/repo/current",
    });

    expect(result.map((r) => r.id)).toEqual(["foreign-heron-6"]);
    expect((await store.get("foreign-heron-6"))?.status).toBe("launched");
  });

  it("returns an empty array when no records exist", async () => {
    const store = mockHandoffStore([]);
    const result = await listHandoffs(store);
    expect(result).toEqual([]);
  });
});
