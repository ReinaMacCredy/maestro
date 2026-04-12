import { describe, expect, it } from "bun:test";
import { parseReleaseVersion, renderVersionFile } from "../../../scripts/version-file";

describe("parseReleaseVersion", () => {
  it("accepts the 0.x.y release scheme", () => {
    expect(parseReleaseVersion("0.25.0")).toEqual({ feature: 25, patch: 0 });
    expect(parseReleaseVersion("0.25.1")).toEqual({ feature: 25, patch: 1 });
  });

  it("rejects non-zero major versions", () => {
    expect(() => parseReleaseVersion("2.4.2")).toThrow(
      "Invalid Maestro release version '2.4.2'. Expected 0.x.y with a zero major version.",
    );
  });

  it("rejects malformed versions", () => {
    expect(() => parseReleaseVersion("0.25")).toThrow(
      "Invalid Maestro release version '0.25'. Expected 0.x.y with a zero major version.",
    );
  });
});

describe("renderVersionFile", () => {
  it("renders the validated version literal", () => {
    const output = renderVersionFile({
      version: "0.25.1",
      buildUnix: 1776024000,
      gitSha: "abc1234",
      releasedAt: "2026-04-12T20:00:00.000Z",
    });

    expect(output).toContain('export const VERSION = "0.25.1";');
    expect(output).toContain("export const BUILD_UNIX = 1776024000;");
    expect(output).toContain('export const GIT_SHA = "abc1234";');
  });
});
