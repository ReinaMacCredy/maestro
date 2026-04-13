import { describe, expect, it } from "bun:test";
import {
  classifyCommitBump,
  splitCommitMessages,
  summarizeCommitBumps,
} from "../../../scripts/auto-bump-lib";

describe("splitCommitMessages", () => {
  it("splits null-delimited git log entries and drops empties", () => {
    expect(
      splitCommitMessages("feat: one\n\nbody\0fix: two\0\0"),
    ).toEqual([
      "feat: one\n\nbody",
      "fix: two",
    ]);
  });
});

describe("classifyCommitBump", () => {
  it("treats feat subjects as feature bumps", () => {
    expect(classifyCommitBump("feat(cli): add command")).toBe("feature");
  });

  it("treats breaking bang subjects as feature bumps", () => {
    expect(classifyCommitBump("fix!: remove old flag")).toBe("feature");
  });

  it("treats BREAKING CHANGE footers as feature bumps", () => {
    expect(
      classifyCommitBump("fix(cli): keep shape\n\nBREAKING CHANGE: output format changed"),
    ).toBe("feature");
  });

  it("keeps ordinary fixes as patch bumps", () => {
    expect(classifyCommitBump("fix(cli): handle empty state")).toBe("patch");
  });
});

describe("summarizeCommitBumps", () => {
  it("counts feature and patch commits from full messages", () => {
    expect(
      summarizeCommitBumps([
        "fix(cli): keep shape\n\nBREAKING CHANGE: output format changed",
        "fix(cli): handle empty state",
      ]),
    ).toEqual({
      bump: "feature",
      featureCount: 1,
      patchCount: 1,
    });
  });
});
