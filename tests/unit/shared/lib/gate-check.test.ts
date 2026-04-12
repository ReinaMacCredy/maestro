import { describe, it, expect } from "bun:test";
import { evaluateGateCheck } from "@/shared/lib/gate-check.js";

describe("evaluateGateCheck", () => {
  describe("array_min_length:N", () => {
    it("passes when array meets minimum length", () => {
      expect(evaluateGateCheck("array_min_length:1", ["one"])).toBe(true);
      expect(evaluateGateCheck("array_min_length:2", ["a", "b"])).toBe(true);
    });

    it("fails when array is too short", () => {
      expect(evaluateGateCheck("array_min_length:2", ["one"])).toBe(false);
      expect(evaluateGateCheck("array_min_length:1", [])).toBe(false);
    });

    it("fails for non-array values", () => {
      expect(evaluateGateCheck("array_min_length:1", "string")).toBe(false);
      expect(evaluateGateCheck("array_min_length:1", null)).toBe(false);
      expect(evaluateGateCheck("array_min_length:1", undefined)).toBe(false);
      expect(evaluateGateCheck("array_min_length:1", 42)).toBe(false);
    });
  });

  describe("object_non_empty", () => {
    it("passes for objects with keys", () => {
      expect(evaluateGateCheck("object_non_empty", { key: "value" })).toBe(true);
      expect(evaluateGateCheck("object_non_empty", { a: 1, b: 2 })).toBe(true);
    });

    it("fails for empty objects", () => {
      expect(evaluateGateCheck("object_non_empty", {})).toBe(false);
    });

    it("fails for non-object values", () => {
      expect(evaluateGateCheck("object_non_empty", null)).toBe(false);
      expect(evaluateGateCheck("object_non_empty", undefined)).toBe(false);
      expect(evaluateGateCheck("object_non_empty", [])).toBe(false);
      expect(evaluateGateCheck("object_non_empty", "string")).toBe(false);
    });
  });

  describe("array_all_passed", () => {
    it("passes when all items have passed: true", () => {
      expect(evaluateGateCheck("array_all_passed", [
        { step: "build", passed: true },
        { step: "test", passed: true },
      ])).toBe(true);
    });

    it("fails when any item has passed: false", () => {
      expect(evaluateGateCheck("array_all_passed", [
        { step: "build", passed: true },
        { step: "test", passed: false },
      ])).toBe(false);
    });

    it("fails for empty array", () => {
      expect(evaluateGateCheck("array_all_passed", [])).toBe(false);
    });

    it("fails for non-array values", () => {
      expect(evaluateGateCheck("array_all_passed", null)).toBe(false);
      expect(evaluateGateCheck("array_all_passed", "string")).toBe(false);
    });

    it("fails for array items without passed field", () => {
      expect(evaluateGateCheck("array_all_passed", [{ step: "build" }])).toBe(false);
    });
  });

  describe("unknown check type", () => {
    it("returns false for unrecognized check types", () => {
      expect(evaluateGateCheck("unknown_check", "anything")).toBe(false);
      expect(evaluateGateCheck("", [])).toBe(false);
    });
  });
});
