import { describe, expect, it } from "bun:test";
import {
  listOpenProjectHandoffIdsForTask,
  listProjectHandoffs,
  showProjectHandoff,
} from "@/features/handoff";
import type { Task } from "@/features/task";
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

describe("handoff read facade", () => {
  it("lists only packets visible to the current project when scoped", async () => {
    const local = makeHandoffRecord({
      id: "local-lark-1",
      createdAt: "2026-04-23T00:00:00.000Z",
      sourceDir: "/repo/current",
    });
    const foreign = makeHandoffRecord({
      id: "foreign-ibis-2",
      createdAt: "2026-04-23T01:00:00.000Z",
      sourceDir: "/repo/other",
    });

    const result = await listProjectHandoffs(mockHandoffStore([foreign, local]), {
      currentProjectRoot: "/repo/current",
    });

    expect(result.map((record) => record.id)).toEqual(["local-lark-1"]);
  });

  it("hides completed linked handoffs from open project lists after reconciliation", async () => {
    const stale = makeHandoffRecord({
      id: "stale-heron-5",
      createdAt: "2026-04-23T00:00:00.000Z",
      refs: { taskId: "tsk-done" },
      sourceDir: "/repo/current",
      status: "launched",
    });
    const open = makeHandoffRecord({
      id: "open-heron-6",
      createdAt: "2026-04-24T00:00:00.000Z",
      sourceDir: "/repo/current",
      status: "launched",
    });
    const store = mockHandoffStore([stale, open]);

    const result = await listProjectHandoffs(store, {
      openOnly: true,
      currentProjectRoot: "/repo/current",
      taskStore: {
        async get(id: string) {
          return id === "tsk-done" ? makeTask(id, "completed") : undefined;
        },
      },
    });

    expect(result.map((record) => record.id)).toEqual(["open-heron-6"]);
    expect((await store.get("stale-heron-5"))?.status).toBe("completed");
  });

  it("does not show a foreign project packet through the scoped detail read", async () => {
    const foreign = makeHandoffRecord({
      id: "foreign-tern-3",
      createdAt: "2026-04-23T00:00:00.000Z",
      sourceDir: "/repo/other",
    });

    await expect(showProjectHandoff(mockHandoffStore([foreign]), "foreign-tern-3", {
      currentProjectRoot: "/repo/current",
    })).rejects.toThrow(MaestroError);
  });

  it("returns open linked packet ids through the same project-scoped behavior", async () => {
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

    const result = await listOpenProjectHandoffIdsForTask(
      mockHandoffStore([local, foreign]),
      "tsk-abc123",
      { currentProjectRoot: "/repo/current" },
    );

    expect(result).toEqual(["local-lark-1"]);
  });
});
