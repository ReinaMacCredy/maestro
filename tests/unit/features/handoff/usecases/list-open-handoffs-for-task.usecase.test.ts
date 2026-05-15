import { describe, expect, it } from "bun:test";
import { listOpenHandoffsForTask } from "@/features/handoff";
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

describe("listOpenHandoffsForTask", () => {
  it("returns open packets whose refs.taskId matches, newest first", async () => {
    const r1 = makeHandoffRecord({
      id: "alpha-fox-1",
      createdAt: "2026-04-20T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
    });
    const r2 = makeHandoffRecord({
      id: "beta-bear-2",
      createdAt: "2026-04-21T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
    });
    const consumed = makeHandoffRecord({
      id: "gamma-owl-3",
      createdAt: "2026-04-22T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
      consumedAt: "2026-04-22T01:00:00.000Z",
    });
    const other = makeHandoffRecord({
      id: "delta-pug-4",
      createdAt: "2026-04-21T12:00:00.000Z",
      refs: { taskId: "tsk-xyz789" },
    });

    const store = mockHandoffStore([r1, r2, consumed, other]);
    const result = await listOpenHandoffsForTask(store, "tsk-abc123", {
      currentProjectRoot: "/src",
    });
    expect(result).toEqual(["beta-bear-2", "alpha-fox-1"]);
  });

  it("returns an empty array when nothing matches", async () => {
    const store = mockHandoffStore([
      makeHandoffRecord({ id: "x-y-1", createdAt: "2026-04-22T00:00:00.000Z", refs: {} }),
    ]);
    const result = await listOpenHandoffsForTask(store, "tsk-missing", {
      currentProjectRoot: "/src",
    });
    expect(result).toEqual([]);
  });

  it("hides stale launched packets whose linked task has already completed", async () => {
    const store = mockHandoffStore([
      makeHandoffRecord({
        id: "stale-ibis-9",
        createdAt: "2026-04-23T00:00:00.000Z",
        refs: { taskId: "tsk-abc123" },
        status: "launched",
      }),
    ]);

    const result = await listOpenHandoffsForTask(store, "tsk-abc123", {
      currentProjectRoot: "/src",
      taskStore: {
        async get(id: string) {
          return id === "tsk-abc123" ? makeTask(id, "completed") : undefined;
        },
      },
    });

    expect(result).toEqual([]);
    expect((await store.get("stale-ibis-9"))?.status).toBe("completed");
  });

  it("does not reconcile stale packets for other tasks", async () => {
    const store = mockHandoffStore([
      makeHandoffRecord({
        id: "stale-ibis-9",
        createdAt: "2026-04-23T00:00:00.000Z",
        refs: { taskId: "tsk-abc123" },
        status: "launched",
      }),
      makeHandoffRecord({
        id: "other-heron-3",
        createdAt: "2026-04-23T01:00:00.000Z",
        refs: { taskId: "tsk-xyz789" },
        status: "launched",
      }),
    ]);

    const result = await listOpenHandoffsForTask(store, "tsk-abc123", {
      currentProjectRoot: "/src",
      taskStore: {
        async get(id: string) {
          return id === "tsk-abc123" || id === "tsk-xyz789"
            ? makeTask(id, "completed")
            : undefined;
        },
      },
    });

    expect(result).toEqual([]);
    expect((await store.get("stale-ibis-9"))?.status).toBe("completed");
    expect((await store.get("other-heron-3"))?.status).toBe("launched");
  });

  it("ignores foreign-project packets that reuse the same task id", async () => {
    const local = makeHandoffRecord({
      id: "local-lark-1",
      createdAt: "2026-04-23T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
      sourceDir: "/repo/current",
    });
    const foreign = makeHandoffRecord({
      id: "foreign-ibis-2",
      createdAt: "2026-04-23T01:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
      sourceDir: "/repo/other",
    });

    const store = mockHandoffStore([local, foreign]);
    const result = await listOpenHandoffsForTask(store, "tsk-abc123", {
      currentProjectRoot: "/repo/current",
    });

    expect(result).toEqual(["local-lark-1"]);
  });
});
