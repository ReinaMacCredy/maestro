import { describe, expect, it } from "bun:test";
import { showHandoff } from "@/features/handoff";
import type { LegacyTask as Task } from "@/shared/domain/legacy-task";
import { MaestroError } from "@/shared/errors.js";
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

describe("showHandoff", () => {
  it("returns the matching packet", async () => {
    const record = makeHandoffRecord({ id: "crimson-fox-1", createdAt: "2026-04-22T00:00:00.000Z" });
    const result = await showHandoff(mockHandoffStore([record]), "crimson-fox-1");
    expect(result.id).toBe("crimson-fox-1");
  });

  it("reconciles a launched packet when its linked task has already completed", async () => {
    const record = makeHandoffRecord({
      id: "amber-otter-2",
      createdAt: "2026-04-22T00:00:00.000Z",
      status: "launched",
      refs: { taskId: "tsk-complete" },
    });
    const store = mockHandoffStore([record]);
    const result = await showHandoff(store, "amber-otter-2", {
      currentProjectRoot: "/src",
      taskStore: {
        async get(id: string) {
          return id === "tsk-complete" ? makeTask(id, "completed") : undefined;
        },
      },
    });

    expect(result.status).toBe("completed");
    expect((await store.get("amber-otter-2"))?.status).toBe("completed");
  });

  it("does not reconcile a foreign-project packet just because the local task id is completed", async () => {
    const record = makeHandoffRecord({
      id: "violet-tern-3",
      createdAt: "2026-04-22T00:00:00.000Z",
      status: "launched",
      refs: { taskId: "tsk-complete" },
      sourceDir: "/repo/other",
    });
    const store = mockHandoffStore([record]);
    const result = await showHandoff(store, "violet-tern-3", {
      currentProjectRoot: "/repo/current",
      taskStore: {
        async get(id: string) {
          return id === "tsk-complete" ? makeTask(id, "completed") : undefined;
        },
      },
    });

    expect(result.status).toBe("launched");
    expect((await store.get("violet-tern-3"))?.status).toBe("launched");
  });

  it("throws MaestroError when the packet does not exist", async () => {
    const store = mockHandoffStore([]);
    await expect(showHandoff(store, "missing-id-9")).rejects.toThrow(MaestroError);
  });
});
