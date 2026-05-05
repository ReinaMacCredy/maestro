import { describe, it, expect } from "bun:test";
import { detectCrossTaskConflict } from "@/features/ci/usecases/detect-cross-task-conflict.js";
import type { DetectCrossTaskConflictInput } from "@/features/ci/usecases/detect-cross-task-conflict.js";

describe("detectCrossTaskConflict", () => {
  describe("happy path — overlapping paths", () => {
    it("detects a single conflicting PR with overlapping files", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["src/foo.ts", "src/bar.ts"],
        otherPrs: [
          { pr: 7, files: ["src/foo.ts", "src/other.ts"] },
        ],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toEqual([7]);
      expect(result.overlappingPaths).toEqual(["src/foo.ts"]);
    });

    it("detects multiple conflicting PRs", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["src/foo.ts", "src/bar.ts", "src/baz.ts"],
        otherPrs: [
          { pr: 7, files: ["src/foo.ts"] },
          { pr: 8, files: ["src/bar.ts"] },
          { pr: 9, files: ["src/unrelated.ts"] },
        ],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toEqual([7, 8]);
      expect(result.overlappingPaths).toEqual(["src/bar.ts", "src/foo.ts"]);
    });
  });

  describe("no-overlap — no conflicts", () => {
    it("returns empty when no other PRs touch the same files", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["src/foo.ts"],
        otherPrs: [
          { pr: 7, files: ["src/completely-different.ts"] },
        ],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toHaveLength(0);
      expect(result.overlappingPaths).toHaveLength(0);
    });

    it("returns empty when otherPrs is empty", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["src/foo.ts"],
        otherPrs: [],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toHaveLength(0);
      expect(result.overlappingPaths).toHaveLength(0);
    });

    it("returns empty when thisPrFiles is empty", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: [],
        otherPrs: [{ pr: 7, files: ["src/foo.ts"] }],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toHaveLength(0);
      expect(result.overlappingPaths).toHaveLength(0);
    });
  });

  describe("multiple-overlap — paths touched by multiple PRs", () => {
    it("deduplicates overlapping paths appearing in more than one conflicting PR", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["src/foo.ts", "src/bar.ts"],
        otherPrs: [
          { pr: 7, files: ["src/foo.ts", "src/bar.ts"] },
          { pr: 8, files: ["src/foo.ts"] },
        ],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.conflictingPrs).toEqual([7, 8]);
      // overlappingPaths should be deduplicated — foo.ts appears in both PRs
      expect(result.overlappingPaths).toEqual(["src/bar.ts", "src/foo.ts"]);
    });
  });

  describe("path-dedup — sorted output", () => {
    it("returns overlappingPaths sorted alphabetically", () => {
      const input: DetectCrossTaskConflictInput = {
        thisPr: 42,
        thisPrFiles: ["z-file.ts", "a-file.ts", "m-file.ts"],
        otherPrs: [
          { pr: 7, files: ["m-file.ts", "a-file.ts", "z-file.ts"] },
        ],
      };
      const result = detectCrossTaskConflict(input);
      expect(result.overlappingPaths).toEqual(["a-file.ts", "m-file.ts", "z-file.ts"]);
    });
  });
});
