import { describe, expect, it } from "bun:test";
import {
  assertNoBlockCycle,
  assertNoParentCycle,
  isTaskPriority,
  isTaskStatus,
  isTaskType,
  validateCreateInput,
  validateTask,
  validateUpdateInput,
} from "@/features/task/domain/task-validators.js";
import { MaestroError } from "@/shared/errors.js";
import type { Task } from "@/features/task/domain/task-types.js";

function fixture(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-a1b2c3",
    title: "Sample",
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-04-10T00:00:00.000Z",
    updatedAt: "2026-04-10T00:00:00.000Z",
    ...overrides,
  };
}

describe("task-validators", () => {
  it("accepts the new task statuses and rejects removed ones", () => {
    expect(isTaskStatus("pending")).toBe(true);
    expect(isTaskStatus("in_progress")).toBe(true);
    expect(isTaskStatus("completed")).toBe(true);
    expect(isTaskStatus("open")).toBe(false);
    expect(isTaskStatus("closed")).toBe(false);
  });

  it("accepts valid task types and priorities", () => {
    expect(isTaskType("task")).toBe(true);
    expect(isTaskType("bug")).toBe(true);
    expect(isTaskPriority(0)).toBe(true);
    expect(isTaskPriority(4)).toBe(true);
    expect(isTaskPriority(5)).toBe(false);
  });

  it("normalizes legacy rows from storage", () => {
    const result = validateTask({
      ...fixture(),
      status: "open",
      dependsOn: ["tsk-000001"],
    });

    expect(result?.status).toBe("pending");
    expect(result?.blockedBy).toEqual(["tsk-000001"]);
    expect(result?.blocks).toEqual([]);
  });

  it("rejects malformed blocker arrays", () => {
    expect(validateTask({ ...fixture(), blockedBy: ["ok", 1] })).toBeUndefined();
    expect(validateTask({ ...fixture(), blocks: "bad" })).toBeUndefined();
  });

  it("validates create input with blocked-by ids", () => {
    const result = validateCreateInput({
      title: "  Hello  ",
      blockedBy: ["tsk-000001"],
    });

    expect(result.title).toBe("Hello");
    expect(result.blockedBy).toEqual(["tsk-000001"]);
  });

  it("rejects malformed blocked-by ids", () => {
    expect(() => validateCreateInput({ title: "X", blockedBy: ["not-an-id"] })).toThrow(MaestroError);
  });

  it("validates update input with completion reason", () => {
    const result = validateUpdateInput({
      title: "  done  ",
      status: "completed",
      reason: "  shipped  ",
    });

    expect(result.title).toBe("done");
    expect(result.reason).toBe("shipped");
  });

  it("rejects invalid updated status", () => {
    expect(() => validateUpdateInput({ status: "open" as never })).toThrow(MaestroError);
  });

  it("rejects parent cycles", () => {
    const tasks = new Map<string, Task>([
      ["tsk-000001", fixture({ id: "tsk-000001" })],
      ["tsk-000002", fixture({ id: "tsk-000002", parentId: "tsk-000001" })],
      ["tsk-000003", fixture({ id: "tsk-000003", parentId: "tsk-000002" })],
    ]);

    expect(() => assertNoParentCycle("tsk-000001", "tsk-000003", tasks)).toThrow(MaestroError);
    expect(() => assertNoParentCycle("tsk-000004", "tsk-000001", tasks)).not.toThrow();
  });

  it("rejects blocker cycles", () => {
    const tasks = new Map<string, Task>([
      ["tsk-000001", fixture({ id: "tsk-000001", blocks: ["tsk-000002"] })],
      ["tsk-000002", fixture({ id: "tsk-000002", blocks: ["tsk-000003"], blockedBy: ["tsk-000001"] })],
      ["tsk-000003", fixture({ id: "tsk-000003", blockedBy: ["tsk-000002"] })],
    ]);

    expect(() => assertNoBlockCycle("tsk-000003", ["tsk-000001"], tasks)).toThrow(MaestroError);
    expect(() => assertNoBlockCycle("tsk-000001", ["tsk-000003"], tasks)).not.toThrow();
  });
});
