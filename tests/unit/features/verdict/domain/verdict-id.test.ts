import { describe, expect, it } from "bun:test";
import {
  VERDICT_ID_PATTERN,
  generateVerdictId,
  isVerdictId,
} from "@/features/verdict/index.js";

describe("verdict-id", () => {
  describe("generateVerdictId", () => {
    it("produces ids matching VERDICT_ID_PATTERN", () => {
      for (let i = 0; i < 50; i++) {
        const id = generateVerdictId();
        expect(id).toMatch(VERDICT_ID_PATTERN);
      }
    });

    it("sorts lexically in chronological order when timestamps increase", () => {
      const t0 = 1_700_000_000_000;
      const ids = [
        generateVerdictId(() => t0),
        generateVerdictId(() => t0 + 1),
        generateVerdictId(() => t0 + 2),
        generateVerdictId(() => t0 + 1_000),
        generateVerdictId(() => t0 + 60_000),
      ];
      const sorted = [...ids].sort();
      expect(sorted).toEqual(ids);
    });

    it("zero-pads short timestamps to width 13 so lexical order matches numeric order", () => {
      const small = generateVerdictId(() => 1);
      const bigger = generateVerdictId(() => 2);
      const muchBigger = generateVerdictId(() => 1_700_000_000_000);
      expect(small < bigger).toBe(true);
      expect(bigger < muchBigger).toBe(true);
      expect(small).toMatch(VERDICT_ID_PATTERN);
    });

    it("rarely collides under repeated calls within the same tick", () => {
      const N = 1000;
      const fixed = () => 1_700_000_000_000;
      const seen = new Set<string>();
      for (let i = 0; i < N; i++) {
        seen.add(generateVerdictId(fixed));
      }
      const collisions = N - seen.size;
      expect(collisions).toBeLessThan(5);
    });
  });

  describe("VERDICT_ID_PATTERN", () => {
    it("rejects malformed ids", () => {
      expect(VERDICT_ID_PATTERN.test("foo")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-abc")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-1234567890123-XYZABC")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("VRD-1714747200123-a1b2c3")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-171474720012-a1b2c3")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-17147472001234-a1b2c3")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-1714747200123-a1b2cg")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("vrd-1714747200123-a1b2c")).toBe(false);
      expect(VERDICT_ID_PATTERN.test("")).toBe(false);
    });
  });

  describe("isVerdictId", () => {
    it("accepts well-formed ids", () => {
      expect(isVerdictId("vrd-1714747200123-a1b2c3")).toBe(true);
      expect(isVerdictId("vrd-0000000000000-000000")).toBe(true);
      expect(isVerdictId("vrd-9999999999999-ffffff")).toBe(true);
    });

    it("returns true for generated ids", () => {
      expect(isVerdictId(generateVerdictId())).toBe(true);
    });

    it("rejects non-verdict ids", () => {
      expect(isVerdictId("evd-1714747200123-a1b2c3")).toBe(false);
      expect(isVerdictId("tsk-a1b2c3")).toBe(false);
      expect(isVerdictId("")).toBe(false);
    });
  });
});
