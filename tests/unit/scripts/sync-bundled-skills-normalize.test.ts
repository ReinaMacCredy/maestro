import { describe, expect, it } from "bun:test";
import { normalizeLineEndings } from "../../../scripts/sync-bundled-skills";

describe("normalizeLineEndings", () => {
  it("normalizes CRLF content to LF for cross-platform generated templates", () => {
    expect(normalizeLineEndings("line one\r\nline two\r\n")).toBe("line one\nline two\n");
  });

  it("preserves LF content as-is", () => {
    expect(normalizeLineEndings("line one\nline two\n")).toBe("line one\nline two\n");
  });
});
