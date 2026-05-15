/**
 * Unit tests for milestone lifecycle usecases
 */
import { describe, expect, it } from "bun:test";
import { listMilestones, getMilestoneStatus, sealMilestone } from "@/features/mission/usecases/milestone-lifecycle.usecase.js";
import { MaestroError } from "@/shared/errors.js";
import type { Mission, Milestone, Feature, Assertion, MissionStatus, MilestoneStatus, FeatureStatus, AssertionResult, MissionStorePort, FeatureStorePort, AssertionStorePort } from "@/shared/domain/legacy-mission";

// Test fixtures
function createTestMission(status: MissionStatus = "draft", milestones: Milestone[] = []): Mission {
  return {
    id: "2024-01-01-001",
    status,
    title: "Test Mission",
    description: "A test mission",
      milestones: milestones.length > 0 ? milestones : [
        { id: "m1", title: "Milestone 1", description: "First", order: 0, featureIds: [] },
        { id: "m2", title: "Milestone 2", description: "Second", order: 1, featureIds: [] },
      ],
    features: ["f1", "f2"],
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  };
}

function createTestFeature(missionId: string, milestoneId: string, status: FeatureStatus = "pending", id = "f1"): Feature {
  return {
    id,
    missionId,
    milestoneId,
    status,
    title: `Feature ${id}`,
    description: "Test feature",
      agentType: "test-skill",
      verificationSteps: ["step1"],
      dependsOn: [],
      fulfills: [],
      createdAt: "2024-01-01T00:00:00Z",
      updatedAt: "2024-01-01T00:00:00Z",
    };
}

function createTestAssertion(missionId: string, milestoneId: string, featureId: string, result: AssertionResult = "pending", id = "a1"): Assertion {
  return {
    id,
    missionId,
    milestoneId,
      featureId,
      result,
      description: `Assertion ${id}`,
      surface: "cli",
      createdAt: "2024-01-01T00:00:00Z",
      updatedAt: "2024-01-01T00:00:00Z",
    };
}

// Mock stores
function createMockMissionStore(mission: Mission | null = null): MissionStorePort {
  return {
    listIds: async () => mission ? [mission.id] : [],
    get: async () => mission ?? undefined,
    exists: async () => mission !== null,
    stage: async () => mission?.id ?? "2024-01-01-001",
    finalize: async () => {},
    update: async () => mission ?? undefined,
    list: async () => mission ? [mission] : [],
  };
}

function createMockFeatureStore(features: Feature[] = []): FeatureStorePort {
  return {
    get: async (missionId, featureId) => features.find(f => f.missionId === missionId && f.id === featureId),
    exists: async (missionId, featureId) => features.some(f => f.missionId === missionId && f.id === featureId),
    create: async (missionId, input, id) => ({ ...input, id, missionId, dependsOn: input.dependsOn ?? [], status: "pending", createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z" } as Feature),
    update: async () => undefined,
    list: async (missionId, filter?: { milestoneId?: string; status?: string }) => {
      let result = features.filter(f => f.missionId === missionId);
      if (filter?.milestoneId) {
        result = result.filter(f => f.milestoneId === filter.milestoneId);
      }
      if (filter?.status) {
        result = result.filter(f => f.status === filter.status);
      }
      return result;
    },
    getMany: async () => [],
  };
}

function createMockAssertionStore(assertions: Assertion[] = []): AssertionStorePort {
  return {
    get: async (missionId, assertionId) => assertions.find(a => a.missionId === missionId && a.id === assertionId),
    exists: async (missionId, assertionId) => assertions.some(a => a.missionId === missionId && a.id === assertionId),
    create: async (missionId, input, id) => ({ ...input, id, result: "pending", createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z" } as Assertion),
    update: async () => undefined,
    list: async (missionId) => assertions.filter(a => a.missionId === missionId),
    listByMilestone: async (missionId, milestoneId) => assertions.filter(a => a.missionId === missionId && a.milestoneId === milestoneId),
    getMany: async () => [],
  };
}

describe("milestone lifecycle usecases", () => {
  describe("listMilestones", () => {
    it("returns all milestones with progress for a mission", async () => {
      const mission = createTestMission("executing");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "done", "f1"),
        createTestFeature(mission.id, "m1", "in-progress", "f2"),
        createTestFeature(mission.id, "m2", "pending", "f3"),
      ];
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f2", "pending", "a2"),
        createTestAssertion(mission.id, "m2", "f3", "pending", "a3"),
      ];

      const result = await listMilestones(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(assertions),
        mission.id,
      );

      expect(result.mission.id).toBe(mission.id);
      expect(result.milestones).toHaveLength(2);
      
      // Milestone 1
      expect(result.milestones[0]!.milestoneId).toBe("m1");
      expect(result.milestones[0]!.featureCount).toBe(2);
      expect(result.milestones[0]!.completedFeatures).toBe(1);
      expect(result.milestones[0]!.featureCompletionPct).toBe(50);
      expect(result.milestones[0]!.assertionCount).toBe(2);
      expect(result.milestones[0]!.passedAssertions).toBe(1);
      
      // Milestone 2
      expect(result.milestones[1]!.milestoneId).toBe("m2");
      expect(result.milestones[1]!.featureCount).toBe(1);
    });

    it("throws for non-existent mission", async () => {
      await expect(
        async () => listMilestones(
          createMockMissionStore(null),
          createMockFeatureStore(),
          createMockAssertionStore(),
          "nonexistent",
        ),
      ).toThrow(MaestroError);
    });

    it("calculates waived assertions correctly", async () => {
      const mission = createTestMission("validating");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f1", "waived", "a2"),
        createTestAssertion(mission.id, "m1", "f1", "waived", "a3"),
      ];

      const result = await listMilestones(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
      );

      expect(result.milestones[0]!.waivedAssertions).toBe(2);
      expect(result.milestones[0]!.waivedAssertionIds).toEqual(["a2", "a3"]);
      expect(result.milestones[0]!.terminalAssertions).toBe(3);
    });

    it("returns 0% completion when no features or assertions", async () => {
      const mission = createTestMission("executing");
      
      const result = await listMilestones(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(),
        mission.id,
      );

      expect(result.milestones[0]!.featureCompletionPct).toBe(0);
      expect(result.milestones[0]!.assertionCompletionPct).toBe(0);
    });

    it("infers executing status from started features even when mission is still approved", async () => {
      const mission = createTestMission("approved");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "in-progress", "f1"),
        createTestFeature(mission.id, "m2", "pending", "f2"),
      ];

      const result = await listMilestones(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(),
        mission.id,
      );

      expect(result.milestones[0]!.status).toBe("executing");
      expect(result.milestones[1]!.status).toBe("pending");
    });
  });

  describe("getMilestoneStatus", () => {
    it("returns detailed status for a specific milestone", async () => {
      const mission = createTestMission("executing");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "done", "f1"),
      ];
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
      ];

      const result = await getMilestoneStatus(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.milestone.id).toBe("m1");
      expect(result.progress.featureCount).toBe(1);
      expect(result.progress.completedFeatures).toBe(1);
    });

    it("throws for non-existent mission", async () => {
      await expect(
        async () => getMilestoneStatus(
          createMockMissionStore(null),
          createMockFeatureStore(),
          createMockAssertionStore(),
          "nonexistent",
          "m1",
        ),
      ).toThrow(MaestroError);
    });

    it("throws for non-existent milestone", async () => {
      const mission = createTestMission();
      
      await expect(
        async () => getMilestoneStatus(
          createMockMissionStore(mission),
          createMockFeatureStore(),
          createMockAssertionStore(),
          mission.id,
          "nonexistent",
        ),
      ).toThrow(MaestroError);
    });
  });

  describe("sealMilestone", () => {
    it("succeeds when all assertions are passed", async () => {
      const mission = createTestMission("validating");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f1", "passed", "a2"),
      ];

      const result = await sealMilestone(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.sealed).toBe(true);
      expect(result.blockingAssertionIds).toHaveLength(0);
      expect(result.progress.terminalAssertions).toBe(2);
    });

    it("succeeds when all assertions are passed or waived", async () => {
      const mission = createTestMission("validating");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f1", "waived", "a2"),
        createTestAssertion(mission.id, "m1", "f2", "waived", "a3"),
      ];

      const result = await sealMilestone(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.sealed).toBe(true);
      expect(result.blockingAssertionIds).toHaveLength(0);
      expect(result.progress.waivedAssertions).toBe(2);
      expect(result.progress.waivedAssertionIds).toContain("a2");
      expect(result.progress.waivedAssertionIds).toContain("a3");
    });

    it("fails and returns blocking assertion IDs for non-terminal assertions", async () => {
      const mission = createTestMission("validating");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f1", "pending", "a2"),
        createTestAssertion(mission.id, "m1", "f2", "failed", "a3"),
        createTestAssertion(mission.id, "m1", "f2", "blocked", "a4"),
      ];

      const result = await sealMilestone(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.sealed).toBe(false);
      expect(result.blockingAssertionIds).toHaveLength(3);
      expect(result.blockingAssertionIds).toContain("a2");
      expect(result.blockingAssertionIds).toContain("a3");
      expect(result.blockingAssertionIds).toContain("a4");
    });

    it("auto-transitions from executing to validating", async () => {
      const mission = createTestMission("executing");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
      ];

      const result = await sealMilestone(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.autoTransitioned).toBe(true);
      expect(result.sealed).toBe(true);
    });

    it("throws for non-existent mission", async () => {
      await expect(
        async () => sealMilestone(
          createMockMissionStore(null),
          createMockFeatureStore(),
          createMockAssertionStore(),
          "nonexistent",
          "m1",
        ),
      ).toThrow(MaestroError);
    });

    it("throws for non-existent milestone", async () => {
      const mission = createTestMission();
      
      await expect(
        async () => sealMilestone(
          createMockMissionStore(mission),
          createMockFeatureStore(),
          createMockAssertionStore(),
          mission.id,
          "nonexistent",
        ),
      ).toThrow(MaestroError);
    });

    it("seals successfully from any status when assertions are terminal", async () => {
      // Even from 'pending' status, seal should work if assertions are terminal
      const mission = createTestMission("draft");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
      ];

      const result = await sealMilestone(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        mission.id,
        "m1",
      );

      expect(result.sealed).toBe(true);
      expect(result.blockingAssertionIds).toHaveLength(0);
    });
  });
});
