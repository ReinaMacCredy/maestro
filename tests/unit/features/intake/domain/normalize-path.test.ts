import { describe, expect, it } from "bun:test";
import { normalizeIntakePath } from "@/features/intake/domain/normalize-path.js";

describe("normalizeIntakePath", () => {
  it("strips leading ./", () => {
    expect(normalizeIntakePath("./skills/foo/SKILL.md", "/repo")).toBe(
      "skills/foo/SKILL.md",
    );
  });

  it("strips cwd prefix for absolute paths under cwd", () => {
    expect(normalizeIntakePath("/repo/.maestro/policies/risk.yaml", "/repo")).toBe(
      ".maestro/policies/risk.yaml",
    );
  });

  it("strips cwd prefix when cwd has trailing slash", () => {
    expect(normalizeIntakePath("/repo/.maestro/x.md", "/repo/")).toBe(".maestro/x.md");
  });

  it("leaves relative paths unchanged", () => {
    expect(normalizeIntakePath("src/foo.ts", "/repo")).toBe("src/foo.ts");
  });

  it("leaves absolute paths outside cwd unchanged", () => {
    expect(normalizeIntakePath("/etc/passwd", "/repo")).toBe("/etc/passwd");
  });

  it("trims whitespace", () => {
    expect(normalizeIntakePath("  src/foo.ts  ", "/repo")).toBe("src/foo.ts");
  });

  it("handles `.` (cwd itself) by not over-stripping", () => {
    expect(normalizeIntakePath(".", "/repo")).toBe(".");
  });

  it("collapses double slashes after dot prefix", () => {
    expect(normalizeIntakePath(".//.maestro/policies/risk.yaml", "/repo")).toBe(
      ".maestro/policies/risk.yaml",
    );
  });

  it("collapses repeated dot-slash segments", () => {
    expect(normalizeIntakePath("././skills/foo", "/repo")).toBe("skills/foo");
  });

  it("returns empty string for empty input", () => {
    expect(normalizeIntakePath("", "/repo")).toBe("");
    expect(normalizeIntakePath("   ", "/repo")).toBe("");
  });
});
