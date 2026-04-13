import { describe, expect, it } from "bun:test";
import { MaestroError } from "@/shared/errors.js";
import {
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
});
