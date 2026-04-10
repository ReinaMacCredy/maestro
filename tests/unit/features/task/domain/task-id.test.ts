import { describe, expect, it } from "bun:test";
import { TASK_ID_PATTERN, generateTaskId, isTaskId } from "@/features/task/domain/task-id.js";

describe("task-id", () => {
  describe("generateTaskId", () => {
    it("produces ids matching the tsk-<6 hex> pattern", () => {
      for (let i = 0; i < 50; i++) {
        const id = generateTaskId();
        expect(id).toMatch(TASK_ID_PATTERN);
      }
    });

    it("produces unique ids on repeated calls", () => {
      const seen = new Set<string>();
      for (let i = 0; i < 1000; i++) {
        seen.add(generateTaskId());
      }
      // 1000 ids out of 16M possibilities should have zero collisions.
      expect(seen.size).toBe(1000);
    });
  });

  describe("isTaskId", () => {
    it("accepts well-formed ids", () => {
      expect(isTaskId("tsk-a1b2c3")).toBe(true);
      expect(isTaskId("tsk-000000")).toBe(true);
      expect(isTaskId("tsk-ffffff")).toBe(true);
    });

    it("rejects malformed ids", () => {
      expect(isTaskId("TSK-a1b2c3")).toBe(false);
      expect(isTaskId("tsk-a1b2c")).toBe(false);
      expect(isTaskId("tsk-a1b2c3d")).toBe(false);
      expect(isTaskId("tsk-A1B2C3")).toBe(false);
      expect(isTaskId("tsk-a1b2cg")).toBe(false);
      expect(isTaskId("feat-a1b2c3")).toBe(false);
      expect(isTaskId("")).toBe(false);
    });
  });
});
