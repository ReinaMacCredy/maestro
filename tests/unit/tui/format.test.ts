import { describe, expect, it } from "bun:test";
import { formatElapsed, formatTokens, formatRelativeTime, truncate } from "../../../src/tui/format.js";

describe("formatElapsed", () => {
  it("formats 0 seconds", () => {
    expect(formatElapsed(0)).toBe("0s");
  });

  it("formats seconds", () => {
    expect(formatElapsed(45_000)).toBe("45s");
  });

  it("formats minutes and seconds", () => {
    expect(formatElapsed(125_000)).toBe("2m 5s");
  });

  it("formats exact minutes", () => {
    expect(formatElapsed(120_000)).toBe("2m");
  });

  it("formats hours and minutes", () => {
    expect(formatElapsed(3_723_000)).toBe("1h 2m");
  });

  it("formats exact hours", () => {
    expect(formatElapsed(7_200_000)).toBe("2h");
  });

  it("clamps negative to 0s", () => {
    expect(formatElapsed(-1000)).toBe("0s");
  });
});

describe("formatTokens", () => {
  it("returns -- for null", () => {
    expect(formatTokens(null)).toBe("--");
  });

  it("formats small numbers as-is", () => {
    expect(formatTokens(500)).toBe("500");
  });

  it("formats thousands as k", () => {
    expect(formatTokens(1500)).toBe("1.5k");
  });

  it("formats millions as M", () => {
    expect(formatTokens(2_500_000)).toBe("2.5M");
  });
});

describe("formatRelativeTime", () => {
  it("formats zero offset as 00:00", () => {
    expect(formatRelativeTime(1000, 1000)).toBe("00:00");
  });

  it("formats minutes", () => {
    const base = 0;
    expect(formatRelativeTime(5 * 60_000, base)).toBe("00:05");
  });

  it("formats hours and minutes", () => {
    const base = 0;
    expect(formatRelativeTime(75 * 60_000, base)).toBe("01:15");
  });

  it("clamps negative to 00:00", () => {
    expect(formatRelativeTime(0, 1000)).toBe("00:00");
  });
});

describe("truncate", () => {
  it("returns text unchanged when shorter than max", () => {
    expect(truncate("hello", 10)).toBe("hello");
  });

  it("truncates with ellipsis", () => {
    expect(truncate("hello world", 8)).toBe("hello...");
  });

  it("handles maxLen 0", () => {
    expect(truncate("hello", 0)).toBe("");
  });

  it("handles maxLen <= 3", () => {
    expect(truncate("hello", 3)).toBe("hel");
  });

  it("returns text unchanged at exact length", () => {
    expect(truncate("hello", 5)).toBe("hello");
  });
});
