import { describe, expect, it } from "bun:test";
import { checkNonEmptyDiff } from "@/features/verify/usecases/checks/check-non-empty-diff.js";

describe("checkNonEmptyDiff", () => {
  it("returns no findings when changedPaths is non-empty", () => {
    const findings = checkNonEmptyDiff({
      changedPaths: ["src/foo.ts"],
      addedLines: [],
      base: "main",
      head: "abc123",
    });
    expect(findings).toEqual([]);
  });

  it("returns no findings when only addedLines is non-empty", () => {
    const findings = checkNonEmptyDiff({
      changedPaths: [],
      addedLines: ["+ console.log('hi')"],
      base: "main",
      head: "abc123",
    });
    expect(findings).toEqual([]);
  });

  it("returns a warn finding when both are empty", () => {
    const findings = checkNonEmptyDiff({
      changedPaths: [],
      addedLines: [],
      base: "main",
      head: "abc123",
    });
    expect(findings).toHaveLength(1);
    expect(findings[0]).toMatchObject({
      check: "empty-diff",
      severity: "warn",
      paths: [],
    });
    expect(findings[0]?.details).toContain("main");
    expect(findings[0]?.details).toContain("abc123");
    expect(findings[0]?.details?.toLowerCase()).toContain("commit");
  });
});
