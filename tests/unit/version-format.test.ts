import { describe, expect, it } from "bun:test";
import { formatRelativeAge, formatVersionOutput } from "../../src/version-format.js";

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
});
