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

  it("base equals head — message points at post-lock commit, not staging", () => {
    // Locking the contract anchors the verifier at the current HEAD.
    // If the agent runs `task verify` before committing any work after
    // lock, base and head are the same SHA and "stage and commit your
    // changes" is misleading — there's nothing staged because no work
    // has been done yet. Direct the agent at the real fix.
    const findings = checkNonEmptyDiff({
      changedPaths: [],
      addedLines: [],
      base: "f8f2c40deadbeef",
      head: "f8f2c40deadbeef",
    });
    expect(findings).toHaveLength(1);
    expect(findings[0]?.details).toContain("base equals HEAD");
    expect(findings[0]?.details?.toLowerCase()).toContain("after locking");
  });
});
