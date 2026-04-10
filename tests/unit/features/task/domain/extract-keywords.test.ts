import { describe, expect, it } from "bun:test";
import { extractKeywords } from "@/features/task/domain/extract-keywords.js";

describe("extractKeywords", () => {
  it("returns an empty array for empty or invalid input", () => {
    expect(extractKeywords("")).toEqual([]);
    expect(extractKeywords("   ")).toEqual([]);
    expect(extractKeywords(null as unknown as string)).toEqual([]);
    expect(extractKeywords(undefined as unknown as string)).toEqual([]);
  });

  it("lowercases and splits on whitespace and punctuation", () => {
    const result = extractKeywords("Argon2 Hashing/Compare, Backwards!");
    expect(result).toContain("argon2");
    expect(result).toContain("hashing");
    expect(result).toContain("compare");
    expect(result).toContain("backwards");
  });

  it("drops stop words", () => {
    const result = extractKeywords("the quick brown fox and the lazy dog");
    expect(result).not.toContain("the");
    expect(result).not.toContain("and");
    expect(result).toContain("quick");
    expect(result).toContain("brown");
    expect(result).toContain("lazy");
  });

  it("drops tokens shorter than 3 characters", () => {
    const result = extractKeywords("a an ab abc abcd");
    expect(result).not.toContain("a");
    expect(result).not.toContain("an");
    expect(result).not.toContain("ab");
    expect(result).toContain("abc");
    expect(result).toContain("abcd");
  });

  it("drops pure-numeric tokens", () => {
    const result = extractKeywords("fix 42 bug and 2026 timeout");
    expect(result).not.toContain("42");
    expect(result).not.toContain("2026");
    expect(result).toContain("fix");
    expect(result).toContain("bug");
    expect(result).toContain("timeout");
  });

  it("dedupes while preserving first-occurrence order", () => {
    const result = extractKeywords("auth login auth token login login");
    expect(result).toEqual(["auth", "login", "token"]);
  });

  it("extracts keywords from a real task title + reason", () => {
    const result = extractKeywords(
      "Implement argon2 password hashing argon2 compare was backwards",
    );
    // Must include the specific tokens we expect to match later.
    expect(result).toContain("argon2");
    expect(result).toContain("password");
    expect(result).toContain("hashing");
    expect(result).toContain("compare");
    expect(result).toContain("backwards");
    // stop words "was" removed.
    expect(result).not.toContain("was");
  });

  it("handles snake_case and kebab-case by splitting on separators", () => {
    const result = extractKeywords("fix_login_endpoint and jwt-middleware");
    expect(result).toContain("fix");
    expect(result).toContain("login");
    expect(result).toContain("endpoint");
    expect(result).toContain("jwt");
    expect(result).toContain("middleware");
  });

  it("returns keywords in input order (no alphabetical sort)", () => {
    const result = extractKeywords("zebra apple mango");
    expect(result).toEqual(["zebra", "apple", "mango"]);
  });
});
