import { describe, expect, it } from "bun:test";
import { wrapText } from "../../../../src/tui/widgets/text-wrap.js";

describe("wrapText", () => {
  it("wraps at word boundary", () => {
    const lines = wrapText("hello world foo bar", 12);
    expect(lines).toEqual(["hello world", "foo bar"]);
  });

  it("truncates with +N more when exceeding maxLines", () => {
    const lines = wrapText("a b c d e f g h", 4, 3);
    expect(lines.length).toBe(3);
    expect(lines[2]).toMatch(/^\+\d+ more$/);
  });

  it("handles single long word", () => {
    const lines = wrapText("abcdefghijklmnop", 8);
    expect(lines[0]).toBe("abcdefgh");
  });

  it("returns empty for empty text", () => {
    expect(wrapText("", 10)).toEqual([]);
  });

  it("returns empty for zero width", () => {
    expect(wrapText("hello", 0)).toEqual([]);
  });

  it("does not wrap when text fits", () => {
    const lines = wrapText("hello world", 20);
    expect(lines).toEqual(["hello world"]);
  });
});
