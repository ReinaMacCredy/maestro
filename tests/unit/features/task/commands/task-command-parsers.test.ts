import { describe, expect, it } from "bun:test";
import { MaestroError } from "@/shared/errors.js";
import {
  parseCreateStatus,
  parseLimit,
  parsePriority,
} from "@/features/task/commands/task-command-parsers.js";

describe("task command parsers", () => {
  describe("parseLimit", () => {
    it("accepts whole-number strings", () => {
      expect(parseLimit("0")).toBe(0);
      expect(parseLimit("20")).toBe(20);
    });

    it("rejects malformed numeric strings", () => {
      for (const value of ["2foo", "1.5", "-1", " 2"]) {
        expect(() => parseLimit(value)).toThrow(MaestroError);
      }
    });
  });

  describe("parsePriority", () => {
    it("accepts valid whole-number priorities", () => {
      expect(parsePriority("0")).toBe(0);
      expect(parsePriority("4")).toBe(4);
    });

    it("rejects malformed numeric strings", () => {
      for (const value of ["1abc", "2.9", "-1", " 3"]) {
        expect(() => parsePriority(value)).toThrow(MaestroError);
      }
    });
  });

  describe("parseCreateStatus", () => {
    it("returns undefined for missing or pending status", () => {
      expect(parseCreateStatus(undefined)).toBeUndefined();
      expect(parseCreateStatus("pending")).toBe("pending");
    });

    it("accepts in_progress for auto-claim on create", () => {
      expect(parseCreateStatus("in_progress")).toBe("in_progress");
    });

    it("rejects completed with a pointed 'create first, complete second' error", () => {
      expect(() => parseCreateStatus("completed")).toThrow(/cannot be created already 'completed'/);
    });

    it("rejects legacy status values with the same error as update", () => {
      for (const value of ["open", "blocked", "deferred", "closed"]) {
        expect(() => parseCreateStatus(value)).toThrow(MaestroError);
      }
    });

    it("rejects unknown status values", () => {
      expect(() => parseCreateStatus("wip")).toThrow(/Invalid --status 'wip'/);
    });
  });
});
