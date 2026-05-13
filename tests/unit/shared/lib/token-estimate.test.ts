import { describe, expect, it } from "bun:test";

import {
  detectShape,
  estimateTokens,
  estimateTokensAuto,
} from "@/shared/lib/token-estimate.js";

describe("estimateTokens", () => {
  it("returns 0 for empty string", () => {
    expect(estimateTokens("")).toBe(0);
  });

  it("uses 4 chars per token for prose", () => {
    expect(estimateTokens("abcdefgh", "prose")).toBe(2);
  });

  it("uses 3.5 chars per token for JSON", () => {
    expect(estimateTokens("0123456", "json")).toBe(2);
  });

  it("rounds up", () => {
    expect(estimateTokens("abcde", "prose")).toBe(2);
  });
});

describe("detectShape", () => {
  it("detects JSON object", () => {
    expect(detectShape('{"a":1}')).toBe("json");
  });

  it("detects JSON array", () => {
    expect(detectShape("[1,2]")).toBe("json");
  });

  it("treats leading whitespace as JSON when content is JSON", () => {
    expect(detectShape('   {"a":1}')).toBe("json");
  });

  it("defaults to prose for plain text", () => {
    expect(detectShape("hello world")).toBe("prose");
  });
});

describe("estimateTokensAuto", () => {
  it("uses JSON ratio for JSON shapes", () => {
    expect(estimateTokensAuto('{"a":1234567}')).toBe(Math.ceil(13 / 3.5));
  });

  it("uses prose ratio for prose", () => {
    expect(estimateTokensAuto("hello world abc")).toBe(Math.ceil(15 / 4));
  });
});
