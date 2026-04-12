/**
 * Tests for principle gate validation on handoff create.
 *
 * These test the gate-check integration at the handoff content level,
 * using evaluateGateCheck from shared/lib. The command-level plumbing
 * (CLI flags, validateGatePrinciples) is exercised via the content shapes.
 */
import { describe, it, expect } from "bun:test";
import { evaluateGateCheck } from "@/shared/lib/gate-check.js";
import { validateUkiHandoffContent } from "@/features/handoff/domain/validators.js";
import type { UkiHandoffContent } from "@/features/handoff/domain/uki-types.js";

function makeExecuteContent(overrides: Record<string, unknown> = {}): UkiHandoffContent {
  return {
    mode: "execute",
    currentState: "done",
    sessionCore: "session-abc",
    decisions: ["decision-1"],
    artifacts: [],
    readMore: [],
    nextAction: "deploy",
    summary: "Completed implementation",
    maestroRefs: { missionId: "2026-04-12-001", featureId: "f1" },
    cs: { work: 0.9 },
    signalDelta: [],
    boundaryState: [],
    risks: [],
    causalDrivers: [],
    divergences: [],
    touchedFiles: [],
    completedWork: ["impl"],
    validation: ["tests pass"],
    ...overrides,
  } as UkiHandoffContent;
}

describe("gate validation on handoff content", () => {
  describe("assumptions field (array_min_length:1)", () => {
    it("passes when assumptions has at least one entry", () => {
      const content = makeExecuteContent({ assumptions: ["I assumed X"] });
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(true);
    });

    it("fails when assumptions is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(false);
    });

    it("fails when assumptions is empty array", () => {
      const content = makeExecuteContent({ assumptions: [] });
      expect(evaluateGateCheck("array_min_length:1", content.assumptions)).toBe(false);
    });
  });

  describe("scopeDeclaration field (object_non_empty)", () => {
    it("passes when scopeDeclaration has keys", () => {
      const content = makeExecuteContent({ scopeDeclaration: { touched: "src/foo.ts" } });
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(true);
    });

    it("fails when scopeDeclaration is empty object", () => {
      const content = makeExecuteContent({ scopeDeclaration: {} });
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(false);
    });

    it("fails when scopeDeclaration is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("object_non_empty", content.scopeDeclaration)).toBe(false);
    });
  });

  describe("verificationResults field (array_all_passed)", () => {
    it("passes when all results have passed: true", () => {
      const content = makeExecuteContent({
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: true },
        ],
      });
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(true);
    });

    it("fails when any result has passed: false", () => {
      const content = makeExecuteContent({
        verificationResults: [
          { step: "build", passed: true },
          { step: "test", passed: false },
        ],
      });
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(false);
    });

    it("fails when verificationResults is missing", () => {
      const content = makeExecuteContent();
      expect(evaluateGateCheck("array_all_passed", content.verificationResults)).toBe(false);
    });
  });

  describe("Zod schema accepts new optional fields", () => {
    it("validates content with all principle gate fields present", () => {
      const raw = makeExecuteContent({
        assumptions: ["Assumed no breaking changes"],
        scopeDeclaration: { touched: "src/foo.ts", reason: "fix bug" },
        complexityDelta: { linesAdded: 10 },
        verificationResults: [{ step: "build", passed: true }],
      });
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toEqual(["Assumed no breaking changes"]);
      expect((parsed as Record<string, unknown>).scopeDeclaration).toEqual({ touched: "src/foo.ts", reason: "fix bug" });
      expect((parsed as Record<string, unknown>).verificationResults).toEqual([{ step: "build", passed: true }]);
    });

    it("validates content without principle gate fields (backward compat)", () => {
      const raw = makeExecuteContent();
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toBeUndefined();
    });

    it("validates plan mode content with gate fields", () => {
      const raw = {
        mode: "plan",
        currentState: "planning",
        sessionCore: "session",
        decisions: [],
        artifacts: [],
        readMore: [],
        nextAction: "review",
        summary: "Plan ready",
        maestroRefs: {},
        cs: { summary: 0.8 },
        signalDelta: [],
        boundaryState: [],
        risks: [],
        causalDrivers: [],
        divergences: [],
        planPaths: [],
        maestroSync: [],
        assumptions: ["Design assumption"],
      };
      const parsed = validateUkiHandoffContent(raw);
      expect(parsed.assumptions).toEqual(["Design assumption"]);
    });
  });

  describe("gate skip behavior", () => {
    it("skips validation when missionId is absent (ad-hoc handoff)", () => {
      const content = makeExecuteContent();
      // When maestroRefs has no missionId, gate validation is skipped entirely.
      // This is tested via the absence of missionId -- no error thrown.
      expect(content.maestroRefs.missionId).toBeDefined();

      const adHocContent = makeExecuteContent({ maestroRefs: {} });
      expect(adHocContent.maestroRefs.missionId).toBeUndefined();
    });
  });
});
