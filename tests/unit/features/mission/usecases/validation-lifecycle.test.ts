/**
 * Unit tests for validation lifecycle usecases
 * Tests: showAssertions, updateAssertion with milestone filtering, evidence persistence, and waived handling
 */
import { describe, expect, it, beforeEach } from "bun:test";
import {
  showAssertions,
  updateAssertion,
} from "@/features/mission/usecases/validation-lifecycle.usecase.js";
import { FsMissionStoreAdapter, FsFeatureStoreAdapter, FsAssertionStoreAdapter } from "@/shared/domain/legacy-mission";
import { MaestroError } from "@/shared/errors.js";
import type { MilestoneInput } from "@/shared/domain/legacy-mission";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp } from "node:fs/promises";

async function createSampleMissionWithAssertions(
  missionStore: FsMissionStoreAdapter,
  featureStore: FsFeatureStoreAdapter,
  assertionStore: FsAssertionStoreAdapter,
): Promise<{ missionId: string; assertions: string[] }> {
  const sampleMilestones: MilestoneInput[] = [
    { id: "m1", title: "Milestone 1", description: "First milestone", order: 0 },
    { id: "m2", title: "Milestone 2", description: "Second milestone", order: 1 },
  ];

  const samplePlan = {
    title: "Test Mission",
    description: "A test mission",
    milestones: sampleMilestones,
    features: [
      {
        id: "f1",
        milestoneId: "m1",
        title: "Feature 1",
        description: "First feature",
        agentType: "test-skill",
        verificationSteps: ["step1", "step2"],
        dependsOn: [],
        fulfills: ["assertion1", "assertion2"],
      },
      {
        id: "f2",
        milestoneId: "m1",
        title: "Feature 2",
        description: "Second feature",
        agentType: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: ["f1"],
        fulfills: ["assertion3"],
      },
      {
        id: "f3",
        milestoneId: "m2",
        title: "Feature 3",
        description: "Third feature",
        agentType: "test-skill",
        verificationSteps: ["step4"],
        dependsOn: [],
        fulfills: ["assertion4"],
      },
    ],
  };

  const { createMission } = await import("@/features/mission/usecases/mission-lifecycle.usecase.js");
  const result = await createMission(missionStore, featureStore, assertionStore, samplePlan);

  const assertions = await assertionStore.list(result.mission.id);

  return {
    missionId: result.mission.id,
    assertions: assertions.map((a) => a.id),
  };
}

describe("validation lifecycle usecases", () => {
  let tmpDir: string;
  let missionStore: FsMissionStoreAdapter;
  let featureStore: FsFeatureStoreAdapter;
  let assertionStore: FsAssertionStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "validation-test-"));
    missionStore = new FsMissionStoreAdapter(tmpDir);
    featureStore = new FsFeatureStoreAdapter(tmpDir);
    assertionStore = new FsAssertionStoreAdapter(tmpDir);
  });

  describe("showAssertions", () => {
    it("returns all assertions for a mission", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const result = await showAssertions(missionStore, assertionStore, missionId);

      expect(result.assertions).toHaveLength(4);
      expect(result.total).toBe(4);
      expect(result.filtered).toBe(4);
    });

    it("filters assertions by milestone", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const result = await showAssertions(missionStore, assertionStore, missionId, "m1");

      expect(result.total).toBe(4);
      expect(result.filtered).toBe(3);
      expect(result.assertions.every((a) => a.milestoneId === "m1")).toBe(true);
    });

    it("returns correct milestone information", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const result = await showAssertions(missionStore, assertionStore, missionId, "m1");

      expect(result.milestoneId).toBe("m1");
      expect(result.assertionCount).toBe(3);
    });

    it("throws for non-existent mission", async () => {
      await expect(
        showAssertions(missionStore, assertionStore, "2026-03-28-001"),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("returns empty array when no assertions match milestone filter", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const result = await showAssertions(missionStore, assertionStore, missionId, "m3");

      expect(result.filtered).toBe(0);
      expect(result.assertions).toHaveLength(0);
    });

    it("preserves assertion ordering by creation date", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const result = await showAssertions(missionStore, assertionStore, missionId);
      const timestamps = result.assertions.map((a) => a.createdAt);

      // Should be sorted by creation date
      for (let i = 1; i < timestamps.length; i++) {
        expect(timestamps[i]! >= timestamps[i - 1]!).toBe(true);
      }
    });
  });

  describe("updateAssertion", () => {
    it("updates assertion status with legal transition", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
      });

      expect(result.assertion.result).toBe("passed");
      expect(result.assertion.id).toBe(assertionId);
    });

    it("persists evidence with result update", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;
      const evidence = "Test output showing success";

      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
        evidence,
      });

      expect(result.assertion.result).toBe("passed");
      expect(result.assertion.evidence).toBe(evidence);
    });

    it("allows waiving with required reason", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;
      const reason = "Not applicable to this implementation";

      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "waived",
        waivedReason: reason,
      });

      expect(result.assertion.result).toBe("waived");
      expect(result.assertion.waivedReason).toBe(reason);
    });

    it("rejects waive without reason", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      await expect(
        updateAssertion(missionStore, assertionStore, missionId, assertionId, {
          result: "waived",
        }),
      ).rejects.toThrow("waivedReason is required when waiving an assertion");
    });

    it("rejects waive with empty reason", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      await expect(
        updateAssertion(missionStore, assertionStore, missionId, assertionId, {
          result: "waived",
          waivedReason: "",
        }),
      ).rejects.toThrow("waivedReason is required when waiving an assertion");
    });

    it("allows retry from failed to pending", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First fail the assertion
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "failed",
        evidence: "Initial failure",
      });

      // Retry: failed -> pending
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "pending",
      });

      expect(result.assertion.result).toBe("pending");
    });

    it("allows retry from blocked to pending", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First block the assertion
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "blocked",
        evidence: "Blocked by external dependency",
      });

      // Retry: blocked -> pending
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "pending",
      });

      expect(result.assertion.result).toBe("pending");
    });

    it("rejects illegal transitions with helpful hints", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First pass the assertion
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
      });

      // Try to go back to pending - should fail
      let threw = false;
      try {
        await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
          result: "pending",
        });
      } catch (err) {
        threw = true;
        expect(err).toBeInstanceOf(MaestroError);
        const me = err as MaestroError;
        expect(me.message).toContain("Invalid assertion transition");
        expect(me.hints.length).toBeGreaterThan(0);
        expect(me.hints.some((h) => h.includes("passed is a terminal state"))).toBe(true);
      }
      expect(threw).toBe(true);
    });

    it("rejects transition from passed to failed", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First pass the assertion
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
      });

      // Try to fail - should fail
      await expect(
        updateAssertion(missionStore, assertionStore, missionId, assertionId, {
          result: "failed",
        }),
      ).rejects.toThrow("Invalid assertion transition");
    });

    it("rejects transition from waived to any state", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First waive the assertion
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "waived",
        waivedReason: "Not applicable",
      });

      // Try to pass - should fail (waived is terminal)
      await expect(
        updateAssertion(missionStore, assertionStore, missionId, assertionId, {
          result: "passed",
        }),
      ).rejects.toThrow("Invalid assertion transition");
    });

    it("preserves existing evidence when only status changes", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First update with evidence
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "failed",
        evidence: "Error log here",
      });

      // Retry without new evidence
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "pending",
      });

      // Evidence should be preserved
      expect(result.assertion.result).toBe("pending");
      expect(result.assertion.evidence).toBe("Error log here");
    });

    it("updates evidence on retry", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // First update with evidence
      await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "failed",
        evidence: "Initial error",
      });

      // Retry with new evidence
      const newEvidence = "Additional debugging info";
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "pending",
        evidence: newEvidence,
      });

      expect(result.assertion.evidence).toBe(newEvidence);
    });

    it("throws for non-existent mission", async () => {
      await expect(
        updateAssertion(missionStore, assertionStore, "2026-03-28-001", "a1", {
          result: "passed",
        }),
      ).rejects.toThrow("Mission 2026-03-28-001 not found");
    });

    it("throws for non-existent assertion", async () => {
      const { missionId } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      await expect(
        updateAssertion(missionStore, assertionStore, missionId, "nonexistent", {
          result: "passed",
        }),
      ).rejects.toThrow("Assertion nonexistent not found");
    });

    it("allows same status update (no-op)", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;
      const before = await assertionStore.get(missionId, assertionId);

      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "pending",
      });

      expect(result.assertion.result).toBe("pending");
      expect(result.assertion.updatedAt).not.toBe(before?.updatedAt);
    });

    it("updates timestamp on every change", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;

      // Initial state
      const before = await assertionStore.get(missionId, assertionId);
      const initialUpdatedAt = before!.updatedAt;

      // Wait a tiny bit to ensure different timestamp
      await new Promise((resolve) => setTimeout(resolve, 10));

      // Update status
      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
      });

      expect(result.assertion.updatedAt).not.toBe(initialUpdatedAt);
    });

    it("preserves all assertion fields during update", async () => {
      const { missionId, assertions } = await createSampleMissionWithAssertions(
        missionStore,
        featureStore,
        assertionStore,
      );

      const assertionId = assertions[0]!;
      const before = await assertionStore.get(missionId, assertionId);

      const result = await updateAssertion(missionStore, assertionStore, missionId, assertionId, {
        result: "passed",
      });

      expect(result.assertion.id).toBe(before!.id);
      expect(result.assertion.missionId).toBe(before!.missionId);
      expect(result.assertion.milestoneId).toBe(before!.milestoneId);
      expect(result.assertion.featureId).toBe(before!.featureId);
      expect(result.assertion.description).toBe(before!.description);
      expect(result.assertion.createdAt).toBe(before!.createdAt);
    });
  });
});
