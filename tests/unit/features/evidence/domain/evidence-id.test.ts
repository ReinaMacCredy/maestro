import { describe, expect, it } from "bun:test";
import {
  EVIDENCE_ID_PATTERN,
  generateEvidenceId,
  isEvidenceId,
} from "@/features/evidence/domain/evidence-id.js";

describe("evidence-id", () => {
  describe("generateEvidenceId", () => {
    it("produces ids matching EVIDENCE_ID_PATTERN", () => {
      for (let i = 0; i < 50; i++) {
        const id = generateEvidenceId();
        expect(id).toMatch(EVIDENCE_ID_PATTERN);
      }
    });

    it("sorts lexically in chronological order when timestamps increase", () => {
      const t0 = 1_700_000_000_000;
      const ids = [
        generateEvidenceId(() => t0),
        generateEvidenceId(() => t0 + 1),
        generateEvidenceId(() => t0 + 2),
        generateEvidenceId(() => t0 + 1_000),
        generateEvidenceId(() => t0 + 60_000),
      ];
      const sorted = [...ids].sort();
      expect(sorted).toEqual(ids);
    });

    it("zero-pads short timestamps to width 13 so lexical order matches numeric order", () => {
      const small = generateEvidenceId(() => 1);
      const bigger = generateEvidenceId(() => 2);
      const muchBigger = generateEvidenceId(() => 1_700_000_000_000);
      expect(small < bigger).toBe(true);
      expect(bigger < muchBigger).toBe(true);
      expect(small).toMatch(EVIDENCE_ID_PATTERN);
    });

    it("rarely collides under repeated calls within the same tick", () => {
      const N = 1000;
      const fixed = () => 1_700_000_000_000;
      const seen = new Set<string>();
      for (let i = 0; i < N; i++) {
        seen.add(generateEvidenceId(fixed));
      }
      const collisions = N - seen.size;
      expect(collisions).toBeLessThan(5);
    });
  });

  describe("isEvidenceId", () => {
    it("accepts well-formed ids", () => {
      expect(isEvidenceId("evd-1714747200123-a1b2c3")).toBe(true);
      expect(isEvidenceId("evd-0000000000000-000000")).toBe(true);
      expect(isEvidenceId("evd-9999999999999-ffffff")).toBe(true);
    });

    it("rejects malformed ids", () => {
      expect(isEvidenceId("EVD-1714747200123-a1b2c3")).toBe(false);
      expect(isEvidenceId("evd-171474720012-a1b2c3")).toBe(false);
      expect(isEvidenceId("evd-17147472001234-a1b2c3")).toBe(false);
      expect(isEvidenceId("evd-1714747200123-A1B2C3")).toBe(false);
      expect(isEvidenceId("evd-1714747200123-a1b2cg")).toBe(false);
      expect(isEvidenceId("evd-1714747200123-a1b2c")).toBe(false);
      expect(isEvidenceId("tsk-a1b2c3")).toBe(false);
      expect(isEvidenceId("")).toBe(false);
    });
  });
});
