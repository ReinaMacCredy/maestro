import { describe, expect, it } from "bun:test";
import type { Task } from "@/features/task/domain/task-types.js";
import { assertNoBlockCycle, validateTask } from "@/features/task/domain/task-validators.js";

function makeTask(id: string, blocks: readonly string[] = []): Task {
  return {
    id,
    title: id,
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks,
    blockedBy: [],
    createdAt: "2026-04-16T00:00:00.000Z",
    updatedAt: "2026-04-16T00:00:00.000Z",
  };
}

describe("assertNoBlockCycle", () => {
  it("handles deep acyclic blocker chains without overflowing the stack", () => {
    const tasks = new Map<string, Task>();
    const depth = 2_000;

    for (let index = 0; index < depth; index++) {
      const id = `tsk-${index.toString(16).padStart(6, "0")}`;
      const nextId = index === depth - 1 ? [] : [`tsk-${(index + 1).toString(16).padStart(6, "0")}`];
      tasks.set(id, makeTask(id, nextId));
    }

    expect(() => assertNoBlockCycle("tsk-a0a0a0", ["tsk-000000"], tasks)).not.toThrow();
  });
});

describe("validateTask", () => {
  it("accepts stored contract pointers and claim anchors", () => {
    const parsed = validateTask({
      id: "tsk-a1b2c3",
      title: "Contract-aware task",
      type: "task",
      priority: 2,
      status: "pending",
      labels: [],
      blocks: [],
      blockedBy: [],
      contractId: "c-a1b2c3",
      claimedAtCommit: "abc123def456",
      createdAt: "2026-04-21T00:00:00.000Z",
      updatedAt: "2026-04-21T00:00:00.000Z",
    });

    expect(parsed).toMatchObject({
      contractId: "c-a1b2c3",
      claimedAtCommit: "abc123def456",
    });
  });
});
