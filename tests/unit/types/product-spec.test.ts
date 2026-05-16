import { describe, expect, it } from "bun:test";
import {
  RISK_CLASSES,
  SPEC_MODES,
  WORK_TYPES,
  isRiskClass,
  isSpecMode,
  isWorkType,
} from "@/types/product-spec.js";
import {
  SPEC_SLUG_PATTERN,
  generateSpecSlug,
  isValidSpecSlug,
} from "@/types/spec-id.js";

describe("WorkType / RiskClass / SpecMode guards", () => {
  it("isWorkType accepts all 6 ADR-0015 values and rejects others", () => {
    for (const v of WORK_TYPES) {
      expect(isWorkType(v)).toBe(true);
    }
    expect(WORK_TYPES.length).toBe(6);
    expect(isWorkType("feature")).toBe(false);
    expect(isWorkType(undefined)).toBe(false);
  });

  it("isRiskClass accepts low/medium/high/critical and rejects others", () => {
    for (const v of RISK_CLASSES) {
      expect(isRiskClass(v)).toBe(true);
    }
    expect(isRiskClass("urgent")).toBe(false);
  });

  it("isSpecMode accepts light/heavy and rejects others", () => {
    for (const v of SPEC_MODES) {
      expect(isSpecMode(v)).toBe(true);
    }
    expect(isSpecMode("medium")).toBe(false);
  });
});

describe("spec slug rules", () => {
  it("SPEC_SLUG_PATTERN accepts kebab-case identifiers in the canonical range", () => {
    expect(SPEC_SLUG_PATTERN.test("improve-handoff-pickup-error")).toBe(true);
    expect(SPEC_SLUG_PATTERN.test("abc")).toBe(true);
    expect(SPEC_SLUG_PATTERN.test("a-b-c")).toBe(true);
  });

  it("SPEC_SLUG_PATTERN rejects leading/trailing hyphens, uppercase, spaces, and consecutive hyphens", () => {
    expect(SPEC_SLUG_PATTERN.test("-leading")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("trailing-")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("Upper-case")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("two  spaces")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("double--hyphen")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("ab")).toBe(false);
    expect(SPEC_SLUG_PATTERN.test("")).toBe(false);
  });

  it("isValidSpecSlug returns false for non-string", () => {
    expect(isValidSpecSlug(undefined)).toBe(false);
    expect(isValidSpecSlug(42)).toBe(false);
    expect(isValidSpecSlug(null)).toBe(false);
  });

  it("generateSpecSlug kebab-cases titles and strips punctuation", () => {
    expect(generateSpecSlug("Improve handoff pickup error")).toBe("improve-handoff-pickup-error");
    expect(generateSpecSlug("Fix: Bug in PARSER!")).toBe("fix-bug-in-parser");
    expect(generateSpecSlug("  leading and trailing  ")).toBe("leading-and-trailing");
    expect(generateSpecSlug("multi    space")).toBe("multi-space");
  });

  it("generateSpecSlug clamps at 64 chars and trims a trailing hyphen if the clamp lands on one", () => {
    const title = "a".repeat(80);
    const slug = generateSpecSlug(title);
    expect(slug.length).toBeLessThanOrEqual(64);
    expect(slug.endsWith("-")).toBe(false);
  });
});
