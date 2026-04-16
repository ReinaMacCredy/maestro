/**
 * Unit tests for mission-report usecase
 */
import { describe, expect, it, beforeEach } from "bun:test";
import { deriveMissionReport, generateMissionReport } from "@/features/mission/usecases/mission-report.usecase.js";
import { MaestroError } from "@/shared/errors.js";
import type { Mission, Feature, Assertion, Milestone, CreateFeatureInput, UpdateFeatureInput, CreateAssertionInput, UpdateAssertionInput } from "@/features/mission/domain/mission-types.js";
import type { MissionStorePort } from "@/features/mission/ports/mission-store.port.js";
import type { FeatureStorePort } from "@/features/mission/feature/ports/feature-store.port.js";
import type { AssertionStorePort } from "@/features/mission/validation/ports/assertion-store.port.js";

// Mock stores
class MockMissionStore implements MissionStorePort {
  private missions: Map<string, Mission> = new Map();

  async listIds(): Promise<readonly string[]> {
    return Array.from(this.missions.keys());
  }

  async get(id: string): Promise<Mission | undefined> {
    return this.missions.get(id);
  }

  async exists(id: string): Promise<boolean> {
    return this.missions.has(id);
  }

  async stage(): Promise<string> {
    return "test-id";
  }

  async finalize(): Promise<void> {}

  async update(id: string, input: Record<string, unknown>): Promise<Mission | undefined> {
    const existing = this.missions.get(id);
    if (!existing) return undefined;
    const updated = { ...existing, ...input } as Mission;
    this.missions.set(id, updated);
    return updated;
  }

  async list(): Promise<readonly Mission[]> {
    return Array.from(this.missions.values());
  }

  // Test helper
  setMission(mission: Mission): void {
    this.missions.set(mission.id, mission);
  }
}

class MockFeatureStore implements FeatureStorePort {
  private features: Map<string, Feature[]> = new Map();

  async get(missionId: string, featureId: string): Promise<Feature | undefined> {
    const missionFeatures = this.features.get(missionId) || [];
    return missionFeatures.find((f) => f.id === featureId);
  }

  async exists(missionId: string, featureId: string): Promise<boolean> {
    const missionFeatures = this.features.get(missionId) || [];
    return missionFeatures.some((f) => f.id === featureId);
  }

  async create(missionId: string, input: CreateFeatureInput, id: string): Promise<Feature> {
    const feature = { ...input, id, missionId, dependsOn: input.dependsOn ?? [], report: undefined } as Feature;
    const existing = this.features.get(missionId) || [];
    this.features.set(missionId, [...existing, feature]);
    return feature;
  }

  async update(
    missionId: string,
    featureId: string,
    input: UpdateFeatureInput,
  ): Promise<Feature | undefined> {
    const missionFeatures = this.features.get(missionId) || [];
    const index = missionFeatures.findIndex((f) => f.id === featureId);
    if (index === -1) return undefined;
    const updated = { ...missionFeatures[index], ...input } as Feature;
    missionFeatures[index] = updated;
    return updated;
  }

  async list(missionId: string, filter?: { milestoneId?: string; status?: string }): Promise<readonly Feature[]> {
    const missionFeatures = this.features.get(missionId) || [];
    let filtered = missionFeatures;
    if (filter?.milestoneId) {
      filtered = filtered.filter((f) => f.milestoneId === filter.milestoneId);
    }
    if (filter?.status) {
      filtered = filtered.filter((f) => f.status === filter.status);
    }
    return filtered;
  }

  async getMany(missionId: string, featureIds: readonly string[]): Promise<readonly Feature[]> {
    const missionFeatures = this.features.get(missionId) || [];
    return missionFeatures.filter((f) => featureIds.includes(f.id));
  }

  // Test helper
  setFeatures(missionId: string, features: Feature[]): void {
    this.features.set(missionId, features);
  }
}

class MockAssertionStore implements AssertionStorePort {
  private assertions: Map<string, Assertion[]> = new Map();

  async get(missionId: string, assertionId: string): Promise<Assertion | undefined> {
    const missionAssertions = this.assertions.get(missionId) || [];
    return missionAssertions.find((a) => a.id === assertionId);
  }

  async exists(missionId: string, assertionId: string): Promise<boolean> {
    const missionAssertions = this.assertions.get(missionId) || [];
    return missionAssertions.some((a) => a.id === assertionId);
  }

  async create(missionId: string, input: CreateAssertionInput, id: string): Promise<Assertion> {
    const assertion = {
      ...input,
      id,
      missionId,
      result: "pending" as const,
      evidence: undefined,
      waivedReason: undefined,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    } as Assertion;
    const existing = this.assertions.get(missionId) || [];
    this.assertions.set(missionId, [...existing, assertion]);
    return assertion;
  }

  async update(
    missionId: string,
    assertionId: string,
    input: UpdateAssertionInput,
  ): Promise<Assertion | undefined> {
    const missionAssertions = this.assertions.get(missionId) || [];
    const index = missionAssertions.findIndex((a) => a.id === assertionId);
    if (index === -1) return undefined;
    const updated = { ...missionAssertions[index], ...input } as Assertion;
    missionAssertions[index] = updated;
    return updated;
  }

  async list(missionId: string): Promise<readonly Assertion[]> {
    return this.assertions.get(missionId) || [];
  }

  async listByMilestone(missionId: string, milestoneId: string): Promise<readonly Assertion[]> {
    const missionAssertions = this.assertions.get(missionId) || [];
    return missionAssertions.filter((a) => a.milestoneId === milestoneId);
  }

  async getMany(missionId: string, assertionIds: readonly string[]): Promise<readonly Assertion[]> {
    const missionAssertions = this.assertions.get(missionId) || [];
    return missionAssertions.filter((a) => assertionIds.includes(a.id));
  }

  // Test helper
  setAssertions(missionId: string, assertions: Assertion[]): void {
    this.assertions.set(missionId, assertions);
  }
}

// Test fixtures
function createTestMission(id: string, status = "draft"): Mission {
  return {
    id,
    status: status as Mission["status"],
    title: "Test Mission",
    description: "Test Description",
      milestones: [
        { id: "m1", title: "Milestone 1", description: "First milestone", order: 0, featureIds: [] },
        { id: "m2", title: "Milestone 2", description: "Second milestone", order: 1, featureIds: [] },
      ],
    features: ["f1", "f2", "f3"],
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}

function createTestFeatures(missionId: string): Feature[] {
  return [
    {
      id: "f1",
      missionId,
      milestoneId: "m1",
      status: "done",
      title: "Feature 1",
      description: "First feature",
        agentType: "test-skill",
        verificationSteps: ["step1"],
        dependsOn: [],
        fulfills: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    {
      id: "f2",
      missionId,
      milestoneId: "m1",
      status: "in-progress",
      title: "Feature 2",
      description: "Second feature",
        agentType: "test-skill",
        verificationSteps: ["step2"],
        dependsOn: ["f1"],
        fulfills: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    {
      id: "f3",
      missionId,
      milestoneId: "m2",
      status: "pending",
      title: "Feature 3",
      description: "Third feature",
        agentType: "test-skill",
        verificationSteps: ["step3"],
        dependsOn: [],
        fulfills: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
  ];
}

function createTestAssertions(missionId: string): Assertion[] {
  return [
    {
      id: "a1",
      missionId,
      milestoneId: "m1",
        featureId: "f1",
        result: "passed",
        description: "Assertion 1",
        surface: "cli",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    {
      id: "a2",
      missionId,
      milestoneId: "m1",
        featureId: "f2",
        result: "pending",
        description: "Assertion 2",
        surface: "cli",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    {
      id: "a3",
      missionId,
      milestoneId: "m2",
        featureId: "f3",
        result: "waived",
        description: "Assertion 3",
        surface: "cli",
        waivedReason: "Not applicable",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
    },
  ];
}

describe("mission-report usecase", () => {
  let missionStore: MockMissionStore;
  let featureStore: MockFeatureStore;
  let assertionStore: MockAssertionStore;

  beforeEach(() => {
    missionStore = new MockMissionStore();
    featureStore = new MockFeatureStore();
    assertionStore = new MockAssertionStore();
  });

  it("throws error for non-existent mission", async () => {
    await expect(
      generateMissionReport(missionStore, featureStore, assertionStore, "nonexistent"),
    ).rejects.toThrow(MaestroError);
  });

  it("returns mission report with progress data", async () => {
    const mission = createTestMission("2024-01-01-001");
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

      const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

      expect(report.mission.id).toBe(mission.id);
      expect(report.effectiveMissionStatus).toBe("draft");
      expect(report.milestones).toHaveLength(2);
      expect(report.summary.totalFeatures).toBe(3);
      expect(report.summary.totalCompletedFeatures).toBe(1);
  });

  it("derives the same report from preloaded mission data", async () => {
    const mission = createTestMission("2024-01-01-001-derive");
    const features = createTestFeatures(mission.id);
    const assertions = createTestAssertions(mission.id);

    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, features);
    assertionStore.setAssertions(mission.id, assertions);

    const generated = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);
    const derived = deriveMissionReport(mission, features, assertions);

    expect(derived).toEqual(generated);
  });

  it("calculates correct milestone progress", async () => {
    const mission = createTestMission("2024-01-01-002");
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

    // Milestone 1: 1/2 features completed = 50%
    const m1 = report.milestones.find((m) => m.milestoneId === "m1");
    expect(m1).toBeDefined();
    expect(m1?.featureCount).toBe(2);
    expect(m1?.completedFeatures).toBe(1);
    expect(m1?.featureCompletionPct).toBe(50);

    // Milestone 1: 1/2 assertions terminal (passed) = 50%
    expect(m1?.assertionCount).toBe(2);
    expect(m1?.terminalAssertions).toBe(1);
    expect(m1?.assertionCompletionPct).toBe(50);

    // Milestone 2: 0/1 features completed = 0%
    const m2 = report.milestones.find((m) => m.milestoneId === "m2");
    expect(m2).toBeDefined();
    expect(m2?.featureCount).toBe(1);
    expect(m2?.completedFeatures).toBe(0);
    expect(m2?.featureCompletionPct).toBe(0);

    // Milestone 2: 1/1 assertions terminal (waived) = 100%
    expect(m2?.assertionCount).toBe(1);
    expect(m2?.terminalAssertions).toBe(1);
    expect(m2?.waivedAssertions).toBe(1);
    expect(m2?.assertionCompletionPct).toBe(100);
  });

  it("calculates overall summary correctly", async () => {
    const mission = createTestMission("2024-01-01-003");
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

    // Overall: 1/3 features completed = 33%
    expect(report.summary.totalFeatures).toBe(3);
    expect(report.summary.totalCompletedFeatures).toBe(1);
    expect(report.summary.overallFeaturePct).toBe(33);

    // Overall: 2/3 assertions terminal = 67%
    expect(report.summary.totalAssertions).toBe(3);
    expect(report.summary.totalTerminalAssertions).toBe(2); // 1 passed + 1 waived
    expect(report.summary.overallAssertionPct).toBe(67);

    // 1 waived assertion
    expect(report.summary.totalWaivedAssertions).toBe(1);
  });

  it("sorts milestones by order", async () => {
    const mission = createTestMission("2024-01-01-004");
    // Reorder milestones to test sorting
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

    expect(report.milestones[0]?.order).toBe(0);
    expect(report.milestones[1]?.order).toBe(1);
    expect(report.milestones[0]?.milestoneId).toBe("m1");
    expect(report.milestones[1]?.milestoneId).toBe("m2");
  });

  it("includes waived assertion IDs in milestone", async () => {
    const mission = createTestMission("2024-01-01-005");
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

    const m2 = report.milestones.find((m) => m.milestoneId === "m2");
    expect(m2).toBeDefined();
    expect(m2?.waivedAssertionIds).toContain("a3");
    expect(m2?.waivedAssertions).toBe(1);
  });

  it("handles mission with no features", async () => {
    const mission: Mission = {
      ...createTestMission("2024-01-01-006"),
      features: [],
    };
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, []);
    assertionStore.setAssertions(mission.id, []);

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

    expect(report.summary.totalFeatures).toBe(0);
    expect(report.summary.overallFeaturePct).toBe(0);
    expect(report.summary.totalAssertions).toBe(0);
    expect(report.summary.overallAssertionPct).toBe(0);
  });

  it("derives correct milestone status based on mission status", async () => {
    const mission = createTestMission("2024-01-01-007", "executing");
    missionStore.setMission(mission);
    featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
    assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

    const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

      // First non-completed milestone should be "executing"
      expect(report.milestones[0]?.status).toBe("executing");
      // Later milestones should remain "pending" until reached
      expect(report.milestones[1]?.status).toBe("pending");
    });

    it("elevates effective mission status when approved work has started", async () => {
      const mission = createTestMission("2024-01-01-008", "approved");
      missionStore.setMission(mission);
      featureStore.setFeatures(mission.id, createTestFeatures(mission.id));
      assertionStore.setAssertions(mission.id, createTestAssertions(mission.id));

      const report = await generateMissionReport(missionStore, featureStore, assertionStore, mission.id);

      expect(report.mission.status).toBe("approved");
      expect(report.effectiveMissionStatus).toBe("executing");
      expect(report.milestones[0]?.status).toBe("executing");
    });
  });
