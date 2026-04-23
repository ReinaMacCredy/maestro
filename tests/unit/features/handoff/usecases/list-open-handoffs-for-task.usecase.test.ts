import { describe, expect, it } from "bun:test";
import { listOpenHandoffsForTask } from "@/features/handoff";
import { makeHandoffLaunchRecord, mockLaunchStore } from "../../../../helpers/mocks.js";

describe("listOpenHandoffsForTask", () => {
  it("returns open packets whose refs.taskId matches, newest first", async () => {
    const r1 = makeHandoffLaunchRecord({
      id: "alpha-fox-1",
      createdAt: "2026-04-20T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
    });
    const r2 = makeHandoffLaunchRecord({
      id: "beta-bear-2",
      createdAt: "2026-04-21T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
    });
    const consumed = makeHandoffLaunchRecord({
      id: "gamma-owl-3",
      createdAt: "2026-04-22T00:00:00.000Z",
      refs: { taskId: "tsk-abc123" },
      consumedAt: "2026-04-22T01:00:00.000Z",
    });
    const other = makeHandoffLaunchRecord({
      id: "delta-pug-4",
      createdAt: "2026-04-21T12:00:00.000Z",
      refs: { taskId: "tsk-xyz789" },
    });

    const store = mockLaunchStore([r1, r2, consumed, other]);
    const result = await listOpenHandoffsForTask(store, "tsk-abc123");
    expect(result).toEqual(["beta-bear-2", "alpha-fox-1"]);
  });

  it("returns an empty array when nothing matches", async () => {
    const store = mockLaunchStore([
      makeHandoffLaunchRecord({ id: "x-y-1", createdAt: "2026-04-22T00:00:00.000Z", refs: {} }),
    ]);
    const result = await listOpenHandoffsForTask(store, "tsk-missing");
    expect(result).toEqual([]);
  });

  it("hides stale launched packets whose linked task has already completed", async () => {
    const store = mockLaunchStore([
      makeHandoffLaunchRecord({
        id: "stale-ibis-9",
        createdAt: "2026-04-23T00:00:00.000Z",
        refs: { taskId: "tsk-abc123" },
        status: "launched",
      }),
    ]);

    const result = await listOpenHandoffsForTask(store, "tsk-abc123", {
      taskStore: {
        async get(id: string) {
          return id === "tsk-abc123" ? { id, status: "completed" } : undefined;
        },
      },
    });

    expect(result).toEqual([]);
    expect((await store.get("stale-ibis-9"))?.status).toBe("completed");
  });
});
