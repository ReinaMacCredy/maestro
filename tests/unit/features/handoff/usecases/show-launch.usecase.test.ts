import { describe, expect, it } from "bun:test";
import { showLaunch } from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";
import { makeHandoffLaunchRecord, mockLaunchStore } from "../../../../helpers/mocks.js";

describe("showLaunch", () => {
  it("returns the matching packet", async () => {
    const record = makeHandoffLaunchRecord({ id: "crimson-fox-1", createdAt: "2026-04-22T00:00:00.000Z" });
    const result = await showLaunch(mockLaunchStore([record]), "crimson-fox-1");
    expect(result.id).toBe("crimson-fox-1");
  });

  it("reconciles a launched packet when its linked task has already completed", async () => {
    const record = makeHandoffLaunchRecord({
      id: "amber-otter-2",
      createdAt: "2026-04-22T00:00:00.000Z",
      status: "launched",
      refs: { taskId: "tsk-complete" },
    });
    const store = mockLaunchStore([record]);
    const result = await showLaunch(store, "amber-otter-2", {
      taskStore: {
        async get(id: string) {
          return id === "tsk-complete" ? { id, status: "completed" } : undefined;
        },
      },
    });

    expect(result.status).toBe("completed");
    expect((await store.get("amber-otter-2"))?.status).toBe("completed");
  });

  it("throws MaestroError when the packet does not exist", async () => {
    const store = mockLaunchStore([]);
    await expect(showLaunch(store, "missing-id-9")).rejects.toThrow(MaestroError);
  });
});
