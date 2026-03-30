/**
 * Unit tests for checkpoint lifecycle usecases
 */
import { describe, expect, it, beforeEach } from "bun:test";
import {
  saveCheckpoint,
  listCheckpoints,
  loadCheckpoint,
} from "../../../src/usecases/checkpoint-lifecycle.usecase.js";
import { MaestroError } from "../../../src/domain/errors.js";
import type {
  Mission,
  Milestone,
  Feature,
  Assertion,
  MissionStatus,
  FeatureStatus,
  AssertionStatus,
  Checkpoint,
} from "../../../src/domain/mission-types.js";
import type { MissionStorePort } from "../../../src/ports/mission-store.port.js";
import type { FeatureStorePort } from "../../../src/ports/feature-store.port.js";
import type { AssertionStorePort } from "../../../src/ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../../../src/ports/checkpoint-store.port.js";

// Test fixtures
function createTestMission(
  status: MissionStatus = "executing",
  milestones: Milestone[] = [],
): Mission {
  return {
    id: "2024-01-01-001",
    status,
    title: "Test Mission",
    description: "A test mission",
    milestones:
      milestones.length > 0
        ? milestones
        : [
            { id: "m1", title: "Milestone 1", description: "First", order: 0 },
            { id: "m2", title: "Milestone 2", description: "Second", order: 1 },
          ],
    features: ["f1", "f2"],
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  };
}

function createTestFeature(
  missionId: string,
  milestoneId: string,
  status: FeatureStatus = "pending",
  id = "f1",
): Feature {
  return {
    id,
    missionId,
    milestoneId,
    status,
    title: `Feature ${id}`,
    description: "Test feature",
    skillName: "test-skill",
    verificationSteps: ["step1"],
    dependsOn: [],
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  };
}

function createTestAssertion(
  missionId: string,
  milestoneId: string,
  featureId: string,
  status: AssertionStatus = "pending",
  id = "a1",
): Assertion {
  return {
    id,
    missionId,
    milestoneId,
    featureId,
    status,
    description: `Assertion ${id}`,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  };
}

// Mock stores
function createMockMissionStore(
  mission: Mission | null = null,
): MissionStorePort {
  return {
    listIds: async () => (mission ? [mission.id] : []),
    get: async () => mission ?? undefined,
    exists: async () => mission !== null,
    stage: async () => mission?.id ?? "2024-01-01-001",
    finalize: async () => {},
    update: async () => mission ?? undefined,
    list: async () => (mission ? [mission] : []),
  };
}

function createMockFeatureStore(features: Feature[] = []): FeatureStorePort {
  const storedFeatures = [...features];
  return {
    get: async (missionId, featureId) =>
      storedFeatures.find(
        (f) => f.missionId === missionId && f.id === featureId,
      ),
    exists: async (missionId, featureId) =>
      storedFeatures.some(
        (f) => f.missionId === missionId && f.id === featureId,
      ),
    create: async (missionId, input, id) =>
      ({
        ...input,
        id,
        missionId,
        dependsOn: input.dependsOn ?? [],
        status: "pending",
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
      }) as Feature,
    update: async (missionId, featureId, input) => {
      const index = storedFeatures.findIndex(
        (f) => f.missionId === missionId && f.id === featureId,
      );
      if (index === -1) return undefined;
      const existing = storedFeatures[index]!;
      const updated = {
        ...existing,
        ...(input.status !== undefined && { status: input.status }),
        ...(input.report !== undefined && { report: input.report }),
      };
      storedFeatures[index] = updated;
      return updated;
    },
    list: async (missionId, filter?: { milestoneId?: string; status?: string }) => {
      let result = storedFeatures.filter((f) => f.missionId === missionId);
      if (filter?.milestoneId) {
        result = result.filter((f) => f.milestoneId === filter.milestoneId);
      }
      if (filter?.status) {
        result = result.filter((f) => f.status === filter.status);
      }
      return result;
    },
    getMany: async () => [],
  };
}

function createMockAssertionStore(
  assertions: Assertion[] = [],
): AssertionStorePort {
  const storedAssertions = [...assertions];
  return {
    get: async (missionId, assertionId) =>
      storedAssertions.find(
        (a) => a.missionId === missionId && a.id === assertionId,
      ),
    exists: async (missionId, assertionId) =>
      storedAssertions.some(
        (a) => a.missionId === missionId && a.id === assertionId,
      ),
    create: async (missionId, input, id) =>
      ({
        ...input,
        id,
        status: "pending",
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
      }) as Assertion,
    update: async (missionId, assertionId, input) => {
      const index = storedAssertions.findIndex(
        (a) => a.missionId === missionId && a.id === assertionId,
      );
      if (index === -1) return undefined;
      const existing = storedAssertions[index]!;
      const updated = {
        ...existing,
        status: input.status,
        evidence: input.evidence,
        waivedReason: input.waivedReason,
      };
      storedAssertions[index] = updated;
      return updated;
    },
    list: async (missionId) =>
      storedAssertions.filter((a) => a.missionId === missionId),
    listByMilestone: async (missionId, milestoneId) =>
      storedAssertions.filter(
        (a) => a.missionId === missionId && a.milestoneId === milestoneId,
      ),
    getMany: async () => [],
  };
}

function createMockCheckpointStore(
  checkpoints: Checkpoint[] = [],
): CheckpointStorePort {
  const storedCheckpoints = [...checkpoints];
  let checkpointId = 0;

  return {
    get: async (missionId, checkpointId) =>
      storedCheckpoints.find(
        (c) => c.missionId === missionId && c.id === checkpointId,
      ),
    save: async (missionId, data) => {
      const id = `checkpoint-${++checkpointId}`;
      const checkpoint: Checkpoint = {
        ...data,
        id,
        missionId,
      };
      storedCheckpoints.push(checkpoint);
      return checkpoint;
    },
    list: async (missionId) =>
      storedCheckpoints
        .filter((c) => c.missionId === missionId)
        .sort(
          (a, b) =>
            new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
        ),
    getLatest: async (missionId) => {
      const sorted = storedCheckpoints
        .filter((c) => c.missionId === missionId)
        .sort(
          (a, b) =>
            new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
        );
      return sorted[0];
    },
    load: async (missionId) => {
      const sorted = storedCheckpoints
        .filter((c) => c.missionId === missionId)
        .sort(
          (a, b) =>
            new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
        );
      return sorted[0];
    },
  };
}

describe("checkpoint lifecycle usecases", () => {
  describe("saveCheckpoint", () => {
    it("saves a checkpoint with current feature and assertion states", async () => {
      const mission = createTestMission("executing");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "completed", "f1"),
        createTestFeature(mission.id, "m1", "in_progress", "f2"),
      ];
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "passed", "a1"),
        createTestAssertion(mission.id, "m1", "f2", "pending", "a2"),
      ];

      const result = await saveCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(assertions),
        createMockCheckpointStore(),
        mission.id,
      );

      expect(result.checkpoint.missionId).toBe(mission.id);
      expect(result.checkpoint.milestoneId).toBe("m1");
      expect(result.checkpoint.timestamp).toBeTruthy();
      expect(result.checkpoint.featureStates).toEqual({
        f1: "completed",
        f2: "in_progress",
      });
      expect(result.checkpoint.assertionStates).toEqual({
        a1: "passed",
        a2: "pending",
      });
    });

    it("throws for non-existent mission", async () => {
      try {
        await saveCheckpoint(
          createMockMissionStore(null),
          createMockFeatureStore(),
          createMockAssertionStore(),
          createMockCheckpointStore(),
          "nonexistent",
        );
        expect(false).toBe(true); // Should not reach here
      } catch (err) {
        expect(err instanceof MaestroError).toBe(true);
      }
    });

    it("captures all feature statuses correctly", async () => {
      const mission = createTestMission("executing");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "pending", "f1"),
        createTestFeature(mission.id, "m1", "in_progress", "f2"),
        createTestFeature(mission.id, "m1", "in_review", "f3"),
        createTestFeature(mission.id, "m1", "completed", "f4"),
        createTestFeature(mission.id, "m1", "blocked", "f5"),
      ];

      const result = await saveCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(),
        createMockCheckpointStore(),
        mission.id,
      );

      expect(result.checkpoint.featureStates).toEqual({
        f1: "pending",
        f2: "in_progress",
        f3: "in_review",
        f4: "completed",
        f5: "blocked",
      });
    });

    it("captures all assertion statuses correctly", async () => {
      const mission = createTestMission("executing");
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "pending", "a1"),
        createTestAssertion(mission.id, "m1", "f1", "passed", "a2"),
        createTestAssertion(mission.id, "m1", "f1", "failed", "a3"),
        createTestAssertion(mission.id, "m1", "f1", "blocked", "a4"),
        createTestAssertion(mission.id, "m1", "f1", "waived", "a5"),
      ];

      const result = await saveCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(assertions),
        createMockCheckpointStore(),
        mission.id,
      );

      expect(result.checkpoint.assertionStates).toEqual({
        a1: "pending",
        a2: "passed",
        a3: "failed",
        a4: "blocked",
        a5: "waived",
      });
    });

    it("handles empty feature and assertion lists", async () => {
      const mission = createTestMission("executing");

      const result = await saveCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(),
        createMockCheckpointStore(),
        mission.id,
      );

      expect(result.checkpoint.featureStates).toEqual({});
      expect(result.checkpoint.assertionStates).toEqual({});
    });

    it("includes timestamp in ISO format", async () => {
      const mission = createTestMission("executing");

      const before = new Date().toISOString();
      const result = await saveCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(),
        createMockCheckpointStore(),
        mission.id,
      );
      const after = new Date().toISOString();

      expect(result.checkpoint.timestamp >= before).toBe(true);
      expect(result.checkpoint.timestamp <= after).toBe(true);
    });
  });

  describe("listCheckpoints", () => {
    it("returns checkpoints sorted newest-first", async () => {
      const mission = createTestMission("executing");
      const checkpoints: Checkpoint[] = [
        {
          id: "cp1",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-01T00:00:00Z",
          featureStates: { f1: "pending" },
          assertionStates: { a1: "pending" },
        },
        {
          id: "cp2",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-02T00:00:00Z",
          featureStates: { f1: "completed" },
          assertionStates: { a1: "passed" },
        },
        {
          id: "cp3",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-03T00:00:00Z",
          featureStates: { f1: "completed" },
          assertionStates: { a1: "passed" },
        },
      ];

      const result = await listCheckpoints(
        createMockMissionStore(mission),
        createMockCheckpointStore(checkpoints),
        mission.id,
      );

      expect(result.mission.id).toBe(mission.id);
      expect(result.checkpoints).toHaveLength(3);
      expect(result.checkpoints[0]!.id).toBe("cp3");
      expect(result.checkpoints[1]!.id).toBe("cp2");
      expect(result.checkpoints[2]!.id).toBe("cp1");
    });

    it("throws for non-existent mission", async () => {
      try {
        await listCheckpoints(
          createMockMissionStore(null),
          createMockCheckpointStore(),
          "nonexistent",
        );
        expect(false).toBe(true); // Should not reach here
      } catch (err) {
        expect(err instanceof MaestroError).toBe(true);
      }
    });

    it("returns empty list when no checkpoints exist", async () => {
      const mission = createTestMission("executing");

      const result = await listCheckpoints(
        createMockMissionStore(mission),
        createMockCheckpointStore(),
        mission.id,
      );

      expect(result.checkpoints).toHaveLength(0);
    });
  });

  describe("loadCheckpoint", () => {
    it("returns the latest checkpoint and restores feature/assertion states", async () => {
      const mission = createTestMission("executing");
      const features: Feature[] = [
        createTestFeature(mission.id, "m1", "in_progress", "f1"),
      ];
      const assertions: Assertion[] = [
        createTestAssertion(mission.id, "m1", "f1", "failed", "a1"),
      ];
      const checkpoints: Checkpoint[] = [
        {
          id: "cp1",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-01T00:00:00Z",
          featureStates: { f1: "pending" },
          assertionStates: { a1: "pending" },
        },
        {
          id: "cp2",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-02T00:00:00Z",
          featureStates: { f1: "completed" },
          assertionStates: { a1: "passed" },
        },
      ];

      const result = await loadCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(features),
        createMockAssertionStore(assertions),
        createMockCheckpointStore(checkpoints),
        mission.id,
      );

      expect(result.checkpoint.id).toBe("cp2");
      expect(result.checkpoint.milestoneId).toBe("m1");
      expect(result.restored.featureCount).toBe(1);
      expect(result.restored.assertionCount).toBe(1);
    });

    it("throws when no checkpoints exist", async () => {
      const mission = createTestMission("executing");

      try {
        await loadCheckpoint(
          createMockMissionStore(mission),
          createMockFeatureStore(),
          createMockAssertionStore(),
          createMockCheckpointStore(),
          mission.id,
        );
        expect(false).toBe(true); // Should not reach here
      } catch (err) {
        expect(err instanceof MaestroError).toBe(true);
      }
    });

    it("throws for non-existent mission", async () => {
      try {
        await loadCheckpoint(
          createMockMissionStore(null),
          createMockFeatureStore(),
          createMockAssertionStore(),
          createMockCheckpointStore(),
          "nonexistent",
        );
        expect(false).toBe(true); // Should not reach here
      } catch (err) {
        expect(err instanceof MaestroError).toBe(true);
      }
    });

    it("checkpoint contains feature and assertion state maps", async () => {
      const mission = createTestMission("executing");
      const checkpoints: Checkpoint[] = [
        {
          id: "cp1",
          missionId: mission.id,
          milestoneId: "m1",
          timestamp: "2024-01-01T00:00:00Z",
          featureStates: {
            f1: "completed",
            f2: "in_progress",
          },
          assertionStates: {
            a1: "passed",
            a2: "waived",
          },
        },
      ];

      const result = await loadCheckpoint(
        createMockMissionStore(mission),
        createMockFeatureStore(),
        createMockAssertionStore(),
        createMockCheckpointStore(checkpoints),
        mission.id,
      );

      expect(result.checkpoint.featureStates).toEqual({
        f1: "completed",
        f2: "in_progress",
      });
      expect(result.checkpoint.assertionStates).toEqual({
        a1: "passed",
        a2: "waived",
      });
    });
  });
});
