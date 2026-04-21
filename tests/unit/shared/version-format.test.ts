import { describe, expect, it } from "bun:test";
import {
  formatRelativeAge,
  formatVersionOutput,
  formatVersionOutputForArgv,
  getVersionMetadata,
  resolveDisplayedGitSha,
} from "@/shared/version-format.js";

describe("version formatting", () => {
  it("formats short relative ages in seconds", () => {
    const now = new Date("2026-04-02T00:10:30.000Z");
    expect(formatRelativeAge("2026-04-02T00:10:00.000Z", now)).toBe("30s ago");
  });

  it("formats medium relative ages in minutes", () => {
    const now = new Date("2026-04-02T01:00:00.000Z");
    expect(formatRelativeAge("2026-04-02T00:11:00.000Z", now)).toBe("49m ago");
  });

  it("formats a build-aware version line", () => {
    const output = formatVersionOutput(
      {
        version: "0.5.0",
        buildUnix: 1_775_123_456,
        gitSha: "e9d9b3",
        releasedAt: "2026-04-01T16:20:52.362Z",
      },
      new Date("2026-04-01T17:09:52.362Z"),
    );

    expect(output).toBe(
      "0.5.0.1775123456-ge9d9b3 (released 2026-04-01T16:20:52.362Z, 49m ago)",
    );
  });

  it("prefers build-time git sha over live and tracked metadata", () => {
    expect(
      resolveDisplayedGitSha({
        buildGitSha: "build123",
        liveGitSha: "live456",
        trackedGitSha: "track789",
      }),
    ).toBe("build123");
  });

  it("falls back to the live repo sha when no build override is present", () => {
    expect(
      getVersionMetadata({}, "live4567").gitSha,
    ).toBe("live4567");
  });

  it("ignores unknown build-time sha overrides", () => {
    expect(
      getVersionMetadata({ MAESTRO_BUILD_GIT_SHA: "unknown" }, "live4567").gitSha,
    ).toBe("live4567");
  });

  it("prefers build-time timestamp overrides over tracked version metadata", () => {
    expect(
      getVersionMetadata({
        MAESTRO_BUILD_UNIX: "1776000000",
        MAESTRO_BUILD_RELEASED_AT: "2026-04-02T11:11:12.000Z",
      }),
    ).toMatchObject({
      buildUnix: 1_776_000_000,
      releasedAt: "2026-04-02T11:11:12.000Z",
    });
  });

  it("ignores malformed MAESTRO_BUILD_UNIX overrides instead of truncating them", () => {
    const baseline = getVersionMetadata({});
    const withMalformedOverride = getVersionMetadata({
      MAESTRO_BUILD_UNIX: "1776000000oops",
      MAESTRO_BUILD_RELEASED_AT: "2026-04-02T11:11:12.000Z",
    });

    expect(withMalformedOverride.buildUnix).toBe(baseline.buildUnix);
  });

  it("avoids live git lookup when version output is not requested", () => {
    const output = formatVersionOutputForArgv(
      ["bun", "src/index.ts", "status"],
      {
        MAESTRO_BUILD_GIT_SHA: "build123",
        MAESTRO_BUILD_UNIX: "1776000000",
        MAESTRO_BUILD_RELEASED_AT: "2026-04-02T11:11:12.000Z",
      },
      new Date("2026-04-01T17:09:52.362Z"),
    );
    expect(output).toContain("-gbuild123 ");
    expect(output).toContain("1776000000");
    expect(output).toContain("released 2026-04-02T11:11:12.000Z");
  });
});
