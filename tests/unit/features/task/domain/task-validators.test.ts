import { describe, expect, it } from "bun:test";
import {
  validateTask,
  validateCreateInput,
  validateUpdateInput,
  assertNoParentCycle,
  isTaskStatus,
  isTaskType,
  isTaskPriority,
} from "@/features/task/domain/task-validators.js";
import { MaestroError } from "@/shared/errors.js";
import type { Task } from "@/features/task/domain/task-types.js";

function fixture(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-a1b2c3",
    title: "Sample",
    type: "task",
    priority: 2,
    status: "open",
    labels: [],
    dependsOn: [],
    createdAt: "2026-04-10T00:00:00.000Z",
    updatedAt: "2026-04-10T00:00:00.000Z",
    ...overrides,
  };
}

describe("task-validators", () => {
  describe("isTaskStatus / isTaskType / isTaskPriority", () => {
    it("accepts all valid values", () => {
      expect(isTaskStatus("open")).toBe(true);
      expect(isTaskStatus("closed")).toBe(true);
      expect(isTaskType("task")).toBe(true);
      expect(isTaskType("bug")).toBe(true);
      expect(isTaskPriority(0)).toBe(true);
      expect(isTaskPriority(4)).toBe(true);
    });

    it("rejects invalid values", () => {
      expect(isTaskStatus("pending")).toBe(false);
      expect(isTaskType("task-ish")).toBe(false);
      expect(isTaskPriority(5)).toBe(false);
      expect(isTaskPriority(-1)).toBe(false);
      expect(isTaskPriority("1")).toBe(false);
    });
  });

  describe("validateTask", () => {
    it("accepts a well-formed task", () => {
      const result = validateTask(fixture());
      expect(result).toBeDefined();
      expect(result?.id).toBe("tsk-a1b2c3");
    });

    it("rejects malformed id", () => {
      expect(validateTask(fixture({ id: "bad-id" }))).toBeUndefined();
    });

    it("rejects empty title", () => {
      expect(validateTask(fixture({ title: "" }))).toBeUndefined();
    });

    it("rejects unknown status", () => {
      expect(validateTask({ ...fixture(), status: "pending" as never })).toBeUndefined();
    });

    it("rejects missing arrays", () => {
      const bad = { ...fixture(), labels: undefined as unknown as readonly string[] };
      expect(validateTask(bad)).toBeUndefined();
    });

    it("returns a fresh object with typed fields", () => {
      const result = validateTask(fixture({ labels: ["urgent"], dependsOn: ["tsk-000001"] }));
      expect(result?.labels).toEqual(["urgent"]);
      expect(result?.dependsOn).toEqual(["tsk-000001"]);
    });
  });

  describe("validateCreateInput", () => {
    it("accepts a minimal input", () => {
      const result = validateCreateInput({ title: "Hello" });
      expect(result.title).toBe("Hello");
    });

    it("trims the title", () => {
      const result = validateCreateInput({ title: "  Hello  " });
      expect(result.title).toBe("Hello");
    });

    it("rejects empty title", () => {
      expect(() => validateCreateInput({ title: "" })).toThrow(MaestroError);
      expect(() => validateCreateInput({ title: "   " })).toThrow(MaestroError);
    });

    it("rejects invalid priority", () => {
      expect(() => validateCreateInput({ title: "X", priority: 9 as never })).toThrow(MaestroError);
    });

    it("rejects malformed depends-on", () => {
      expect(() => validateCreateInput({ title: "X", dependsOn: ["not-an-id"] })).toThrow(MaestroError);
    });

    it("rejects malformed parent", () => {
      expect(() => validateCreateInput({ title: "X", parentId: "bad" })).toThrow(MaestroError);
    });
  });

  describe("validateUpdateInput", () => {
    it("accepts partial updates", () => {
      const result = validateUpdateInput({ title: "new" });
      expect(result.title).toBe("new");
    });

    it("trims updated title", () => {
      const result = validateUpdateInput({ title: "  new  " });
      expect(result.title).toBe("new");
    });

    it("accepts empty parent to clear", () => {
      expect(() => validateUpdateInput({ parentId: "" })).not.toThrow();
    });

    it("rejects empty title on update", () => {
      expect(() => validateUpdateInput({ title: "" })).toThrow(MaestroError);
    });

    it("rejects unknown status", () => {
      expect(() => validateUpdateInput({ status: "pending" as never })).toThrow(MaestroError);
    });
  });

  describe("assertNoParentCycle", () => {
    const tasks = new Map<string, Task>([
      ["tsk-000001", fixture({ id: "tsk-000001", parentId: undefined })],
      ["tsk-000002", fixture({ id: "tsk-000002", parentId: "tsk-000001" })],
      ["tsk-000003", fixture({ id: "tsk-000003", parentId: "tsk-000002" })],
    ]);

    it("allows parenting a new leaf under an existing root", () => {
      expect(() => assertNoParentCycle("tsk-000004", "tsk-000001", tasks)).not.toThrow();
    });

    it("rejects parenting a task under itself", () => {
      expect(() => assertNoParentCycle("tsk-000001", "tsk-000001", tasks)).toThrow(MaestroError);
    });

    it("rejects parenting an ancestor under a descendant (would create a cycle)", () => {
      // tsk-000001 cannot be parented under tsk-000003 because tsk-000003 -> tsk-000002 -> tsk-000001
      expect(() => assertNoParentCycle("tsk-000001", "tsk-000003", tasks)).toThrow(MaestroError);
    });

    it("allows unrelated sibling parenting", () => {
      const siblings = new Map<string, Task>([
        ["tsk-aaaaaa", fixture({ id: "tsk-aaaaaa" })],
        ["tsk-bbbbbb", fixture({ id: "tsk-bbbbbb" })],
      ]);
      expect(() => assertNoParentCycle("tsk-aaaaaa", "tsk-bbbbbb", siblings)).not.toThrow();
    });
  });
});
