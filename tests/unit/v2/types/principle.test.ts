import { describe, expect, it } from "bun:test";
import {
  isValidPrincipleSlug,
  PRINCIPLE_SLUG_PATTERN,
} from "@/v2/types/principle.js";

describe("PRINCIPLE_SLUG_PATTERN", () => {
  it("accepts kebab-case", () => {
    expect(PRINCIPLE_SLUG_PATTERN.test("prefer-shared-utils")).toBe(true);
  });
  it("accepts digits", () => {
    expect(PRINCIPLE_SLUG_PATTERN.test("rule-42")).toBe(true);
  });
  it("rejects underscores", () => {
    expect(PRINCIPLE_SLUG_PATTERN.test("prefer_shared")).toBe(false);
  });
  it("rejects leading/trailing dash", () => {
    expect(PRINCIPLE_SLUG_PATTERN.test("-x")).toBe(false);
    expect(PRINCIPLE_SLUG_PATTERN.test("x-")).toBe(false);
  });
});

describe("isValidPrincipleSlug", () => {
  it("requires length 2..64", () => {
    expect(isValidPrincipleSlug("a")).toBe(false);
    expect(isValidPrincipleSlug("ab")).toBe(true);
    expect(isValidPrincipleSlug("a".repeat(65))).toBe(false);
  });
  it("rejects non-strings", () => {
    expect(isValidPrincipleSlug(undefined)).toBe(false);
    expect(isValidPrincipleSlug(42)).toBe(false);
  });
});
