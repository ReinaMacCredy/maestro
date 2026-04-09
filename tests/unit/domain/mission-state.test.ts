import { describe, expect, it } from "bun:test";
import { MaestroError } from "@/domain/errors.js";
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
} from "@/domain/mission-state.js";

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

  // ============================
  // Phase 7 additions
  // ============================

  describe("paused mission state", () => {
    it("allows executing -> paused", () => {
      expect(canTransitionMission("executing", "paused")).toBe(true);
    });

    it("allows paused -> executing (resume)", () => {
      expect(canTransitionMission("paused", "executing")).toBe(true);
    });

    it("rejects paused -> completed (must resume first)", () => {
      expect(canTransitionMission("paused", "completed")).toBe(false);
    });

    it("paused has exactly one outbound transition (executing)", () => {
      const valid = getValidMissionTransitions("paused");
      expect(valid).toEqual(["executing"]);
    });

    it("assertMissionTransition throws for paused -> completed", () => {
      expect(() => assertMissionTransition("paused", "completed")).toThrow(MaestroError);
    });

    it("assertMissionTransition allows executing -> paused", () => {
      expect(() => assertMissionTransition("executing", "paused")).not.toThrow();
    });
  });

  describe("assigned feature state", () => {
    it("allows pending -> assigned", () => {
      expect(canTransitionFeature("pending", "assigned")).toBe(true);
    });

    it("allows assigned -> in-progress", () => {
      expect(canTransitionFeature("assigned", "in-progress")).toBe(true);
    });

    it("rejects assigned -> review (must go through in-progress first)", () => {
      expect(canTransitionFeature("assigned", "review")).toBe(false);
    });

    it("assigned has exactly one outbound transition (in-progress)", () => {
      const valid = getValidFeatureTransitions("assigned");
      expect(valid).toEqual(["in-progress"]);
    });

    it("assertFeatureTransition throws for assigned -> review", () => {
      expect(() => assertFeatureTransition("assigned", "review")).toThrow(MaestroError);
    });

    it("assertFeatureTransition allows pending -> assigned", () => {
      expect(() => assertFeatureTransition("pending", "assigned")).not.toThrow();
    });
  });

  describe("blocked -> waived assertion transition (B1 fix)", () => {
    it("allows blocked -> waived", () => {
      expect(canTransitionAssertion("blocked", "waived")).toBe(true);
    });

    it("assertAssertionTransition allows blocked -> waived", () => {
      expect(() => assertAssertionTransition("blocked", "waived")).not.toThrow();
    });

    it("blocked has two outbound transitions: pending and waived", () => {
      const valid = getValidAssertionTransitions("blocked");
      expect(valid).toContain("pending");
      expect(valid).toContain("waived");
      expect(valid).toHaveLength(2);
    });
  });

  describe("validating -> executing milestone retry", () => {
    it("allows validating -> executing (retry after failed validation)", () => {
      expect(canTransitionMilestone("validating", "executing")).toBe(true);
    });

    it("assertMilestoneTransition allows validating -> executing", () => {
      expect(() => assertMilestoneTransition("validating", "executing")).not.toThrow();
    });

    it("validating has three outbound transitions", () => {
      const valid = getValidMilestoneTransitions("validating");
      expect(valid).toContain("sealed");
      expect(valid).toContain("failed");
      expect(valid).toContain("executing");
      expect(valid).toHaveLength(3);
    });
  });

  describe("sealed as terminal milestone state", () => {
    it("sealed has no outbound transitions", () => {
      const valid = getValidMilestoneTransitions("sealed");
      expect(valid).toHaveLength(0);
    });

    it("rejects sealed -> pending", () => {
      expect(canTransitionMilestone("sealed", "pending")).toBe(false);
    });

    it("rejects sealed -> executing", () => {
      expect(canTransitionMilestone("sealed", "executing")).toBe(false);
    });

    it("rejects sealed -> validating", () => {
      expect(canTransitionMilestone("sealed", "validating")).toBe(false);
    });

    it("rejects sealed -> failed", () => {
      expect(canTransitionMilestone("sealed", "failed")).toBe(false);
    });

    it("assertMilestoneTransition throws with terminal hint for sealed", () => {
      try {
        assertMilestoneTransition("sealed", "executing");
      } catch (err) {
        expect(err).toBeInstanceOf(MaestroError);
        const error = err as MaestroError;
        expect(error.hints.some((h: string) => h.includes("terminal"))).toBe(true);
      }
    });
  });

  describe("done as terminal feature state", () => {
    it("done has no outbound transitions", () => {
      const valid = getValidFeatureTransitions("done");
      expect(valid).toHaveLength(0);
    });

    it("rejects done -> pending", () => {
      expect(canTransitionFeature("done", "pending")).toBe(false);
    });

    it("rejects done -> assigned", () => {
      expect(canTransitionFeature("done", "assigned")).toBe(false);
    });

    it("rejects done -> in-progress", () => {
      expect(canTransitionFeature("done", "in-progress")).toBe(false);
    });

    it("rejects done -> review", () => {
      expect(canTransitionFeature("done", "review")).toBe(false);
    });

    it("rejects done -> blocked", () => {
      expect(canTransitionFeature("done", "blocked")).toBe(false);
    });

    it("assertFeatureTransition throws with terminal hint for done", () => {
      try {
        assertFeatureTransition("done", "pending");
      } catch (err) {
        expect(err).toBeInstanceOf(MaestroError);
        const error = err as MaestroError;
        expect(error.hints.some((h: string) => h.includes("terminal"))).toBe(true);
      }
    });
  });
});
