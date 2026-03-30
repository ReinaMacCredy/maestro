import { describe, expect, it } from "bun:test";
import { MaestroError } from "../../../src/domain/errors.js";
import {
  assertMissionTransition,
  assertMilestoneTransition,
  assertFeatureTransition,
  assertAssertionTransition,
  canTransitionMission,
  canTransitionMilestone,
  canTransitionFeature,
  canTransitionAssertion,
  getValidMissionTransitions,
  getValidMilestoneTransitions,
  getValidFeatureTransitions,
  getValidAssertionTransitions,
} from "../../../src/domain/mission-state.js";

describe("mission state transitions", () => {
  describe("mission transitions", () => {
    it("allows draft -> approved", () => {
      expect(canTransitionMission("draft", "approved")).toBe(true);
    });

    it("allows draft -> rejected", () => {
      expect(canTransitionMission("draft", "rejected")).toBe(true);
    });

    it("allows approved -> executing", () => {
      expect(canTransitionMission("approved", "executing")).toBe(true);
    });

    it("allows executing -> validating", () => {
      expect(canTransitionMission("executing", "validating")).toBe(true);
    });

    it("allows validating -> completed", () => {
      expect(canTransitionMission("validating", "completed")).toBe(true);
    });

    it("allows validating -> failed", () => {
      expect(canTransitionMission("validating", "failed")).toBe(true);
    });

    it("rejects draft -> executing directly", () => {
      expect(canTransitionMission("draft", "executing")).toBe(false);
    });

    it("rejects completed -> any state", () => {
      expect(canTransitionMission("completed", "draft")).toBe(false);
      expect(canTransitionMission("completed", "approved")).toBe(false);
      expect(canTransitionMission("completed", "executing")).toBe(false);
    });

    it("rejects rejected -> any state", () => {
      expect(canTransitionMission("rejected", "draft")).toBe(false);
      expect(canTransitionMission("rejected", "approved")).toBe(false);
    });

    it("returns valid next states for draft", () => {
      const valid = getValidMissionTransitions("draft");
      expect(valid).toContain("approved");
      expect(valid).toContain("rejected");
      expect(valid).not.toContain("executing");
    });

    it("returns valid next states for approved", () => {
      const valid = getValidMissionTransitions("approved");
      expect(valid).toContain("executing");
      expect(valid).not.toContain("draft");
    });

    it("assertMissionTransition throws MaestroError with hints on invalid transition", () => {
      expect(() => assertMissionTransition("draft", "executing")).toThrow(MaestroError);
      try {
        assertMissionTransition("draft", "executing");
      } catch (err) {
        expect(err).toBeInstanceOf(MaestroError);
        const error = err as MaestroError;
        expect(error.hints.length).toBeGreaterThan(0);
        expect(error.hints.some((h: string) => h.includes("approved") || h.includes("rejected"))).toBe(true);
      }
    });

    it("assertMissionTransition does not throw for valid transition", () => {
      expect(() => assertMissionTransition("draft", "approved")).not.toThrow();
    });
  });

  describe("milestone transitions", () => {
    it("allows pending -> executing", () => {
      expect(canTransitionMilestone("pending", "executing")).toBe(true);
    });

    it("allows executing -> validating", () => {
      expect(canTransitionMilestone("executing", "validating")).toBe(true);
    });

    it("allows validating -> sealed", () => {
      expect(canTransitionMilestone("validating", "sealed")).toBe(true);
    });

    it("allows validating -> failed", () => {
      expect(canTransitionMilestone("validating", "failed")).toBe(true);
    });

    it("rejects pending -> validating directly", () => {
      expect(canTransitionMilestone("pending", "validating")).toBe(false);
    });

    it("rejects sealed -> any state", () => {
      expect(canTransitionMilestone("sealed", "pending")).toBe(false);
      expect(canTransitionMilestone("sealed", "executing")).toBe(false);
    });

    it("assertMilestoneTransition throws MaestroError with hints on invalid transition", () => {
      expect(() => assertMilestoneTransition("pending", "validating")).toThrow(MaestroError);
    });
  });

  describe("feature transitions", () => {
    it("allows pending -> in-progress", () => {
      expect(canTransitionFeature("pending", "in-progress")).toBe(true);
    });

    it("allows in-progress -> review", () => {
      expect(canTransitionFeature("in-progress", "review")).toBe(true);
    });

    it("allows review -> done", () => {
      expect(canTransitionFeature("review", "done")).toBe(true);
    });

    it("allows review -> blocked", () => {
      expect(canTransitionFeature("review", "blocked")).toBe(true);
    });

    it("allows blocked -> pending (retry)", () => {
      expect(canTransitionFeature("blocked", "pending")).toBe(true);
    });

    it("allows review -> pending (retry)", () => {
      expect(canTransitionFeature("review", "pending")).toBe(true);
    });

    it("rejects pending -> done directly", () => {
      expect(canTransitionFeature("pending", "done")).toBe(false);
    });

    it("rejects done -> any state", () => {
      expect(canTransitionFeature("done", "pending")).toBe(false);
      expect(canTransitionFeature("done", "in-progress")).toBe(false);
    });

    it("assertFeatureTransition throws MaestroError with hints on invalid transition", () => {
      expect(() => assertFeatureTransition("pending", "done")).toThrow(MaestroError);
    });
  });

  describe("assertion transitions", () => {
    it("allows pending -> passed", () => {
      expect(canTransitionAssertion("pending", "passed")).toBe(true);
    });

    it("allows pending -> failed", () => {
      expect(canTransitionAssertion("pending", "failed")).toBe(true);
    });

    it("allows pending -> blocked", () => {
      expect(canTransitionAssertion("pending", "blocked")).toBe(true);
    });

    it("allows pending -> waived", () => {
      expect(canTransitionAssertion("pending", "waived")).toBe(true);
    });

    it("allows failed -> pending (retry)", () => {
      expect(canTransitionAssertion("failed", "pending")).toBe(true);
    });

    it("allows blocked -> pending (retry)", () => {
      expect(canTransitionAssertion("blocked", "pending")).toBe(true);
    });

    it("waived is terminal - no transitions out", () => {
      expect(canTransitionAssertion("waived", "pending")).toBe(false);
      expect(canTransitionAssertion("waived", "passed")).toBe(false);
      expect(canTransitionAssertion("waived", "failed")).toBe(false);
    });

    it("passed is terminal - no transitions out", () => {
      expect(canTransitionAssertion("passed", "pending")).toBe(false);
      expect(canTransitionAssertion("passed", "failed")).toBe(false);
    });

    it("assertAssertionTransition throws MaestroError with hints on invalid transition", () => {
      expect(() => assertAssertionTransition("waived", "pending")).toThrow(MaestroError);
    });

    it("assertAssertionTransition allows failed or blocked assertions to return to pending", () => {
      expect(() => assertAssertionTransition("failed", "pending")).not.toThrow();
      expect(() => assertAssertionTransition("blocked", "pending")).not.toThrow();
    });
  });
});
