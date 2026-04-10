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

    it("produces near-unique ids on repeated calls", () => {
      // 6-hex ids draw from a 16M space. Birthday paradox says 1000 picks
      // produce a ~3% chance of at least one collision, so we cannot assert
      // strict uniqueness. Assert that collisions are rare (<0.5% of draws)
      // rather than zero, which is the real guarantee at this id length.
      const N = 1000;
      const seen = new Set<string>();
      for (let i = 0; i < N; i++) {
        seen.add(generateTaskId());
      }
      const collisions = N - seen.size;
      expect(collisions).toBeLessThan(5);
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
