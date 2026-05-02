import { describe, expect, it } from "bun:test";
import { buildMissions } from "@/features/mission/usecases/missions.usecase.js";
import type {
  Assertion,
  Checkpoint,
  Feature,
  FeatureStatus,
  Mission,
  MissionStatus,
} from "@/features/mission";
import { MaestroError } from "@/shared/errors.js";
import {
  mockAssertionStore,
  mockCheckpointStore,
  mockFeatureStore,
  mockMissions,
  mockMissionStore,
} from "../../../../helpers/mocks.js";

const CREATED = "2026-01-01T00:00:00.000Z";

function makeMission(
  id: string,
  status: MissionStatus = "draft",
  createdAt = CREATED,
): Mission {
  return {
    id,
    status,
    title: `Mission ${id}`,
    description: "Mission description",
    milestones: [
      {
        id: "m1",
        title: "Milestone 1",
        description: "First milestone",
        order: 0,
        featureIds: ["f1", "f2"],
      },
      {
        id: "m2",
        title: "Milestone 2",
        description: "Second milestone",
        order: 1,
        featureIds: ["f3"],
      },
    ],
    features: ["f1", "f2", "f3"],
    createdAt,
    updatedAt: createdAt,
  };
}

function makeFeature(
  id: string,
  status: FeatureStatus,
  milestoneId = "m1",
): Feature {
  return {
    id,
    missionId: "mission-001",
    milestoneId,
    status,
    title: `Feature ${id}`,
    description: "Feature description",
    agentType: "test-skill",
    verificationSteps: ["verify it"],
    dependsOn: [],
    fulfills: [`a-${id}`],
    createdAt: CREATED,
    updatedAt: CREATED,
  };
}

function makeAssertion(
  id: string,
  featureId: string,
  milestoneId = "m1",
): Assertion {
  return {
    id,
    missionId: "mission-001",
    milestoneId,
    featureId,
    result: "pending",
    description: `Assertion ${id}`,
    surface: "cli",
    createdAt: CREATED,
    updatedAt: CREATED,
  };
}

function makeCheckpoint(id: string): Checkpoint {
  return {
    id,
    missionId: "mission-001",
    currentMilestoneId: "m1",
    timestamp: CREATED,
    featureStatuses: { f1: "pending" },
    assertionResults: { "a-f1": "pending" },
  };
}

describe("Missions", () => {
  it("loadFullState returns the mission and every read slice", async () => {
    const mission = makeMission("mission-001");
    const features = [makeFeature("f1", "pending")];
    const assertions = [makeAssertion("a-f1", "f1")];
    const checkpoints = [makeCheckpoint("checkpoint-1")];
    const missions = buildMissions(
      mockMissionStore([mission]),
      mockFeatureStore(mission.id, features),
      mockAssertionStore(mission.id, assertions),
      mockCheckpointStore(mission.id, checkpoints),
    );

    await expect(missions.loadFullState(mission.id)).resolves.toEqual({
      mission,
      features,
      assertions,
      checkpoints,
    });
  });

  it("loadFullState throws a MaestroError when the mission is missing", async () => {
    const missions = buildMissions(
      mockMissionStore(),
      mockFeatureStore("missing"),
      mockAssertionStore("missing"),
      mockCheckpointStore("missing"),
    );

    try {
      await missions.loadFullState("missing");
      throw new Error("expected loadFullState to throw");
    } catch (error) {
      expect(error).toBeInstanceOf(MaestroError);
      expect((error as MaestroError).message).toBe("Mission missing not found");
      expect((error as MaestroError).hints).toContain("List missions: maestro mission list");
    }
  });

  it("resolveMissionId returns explicit IDs as-is", async () => {
    const missions = buildMissions(
      mockMissionStore(),
      mockFeatureStore("unused"),
      mockAssertionStore("unused"),
      mockCheckpointStore("unused"),
    );

    await expect(missions.resolveMissionId("mission-does-not-exist")).resolves.toBe("mission-does-not-exist");
  });

  it("resolveMissionId prefers executing or paused missions before newest fallback", async () => {
    const missions = buildMissions(
      mockMissionStore([
        makeMission("mission-001", "draft", "2026-01-01T00:00:00.000Z"),
        makeMission("mission-002", "executing", "2026-01-02T00:00:00.000Z"),
        makeMission("mission-003", "draft", "2026-01-03T00:00:00.000Z"),
      ]),
      mockFeatureStore("mission-002"),
      mockAssertionStore("mission-002"),
      mockCheckpointStore("mission-002"),
    );

    await expect(missions.resolveMissionId()).resolves.toBe("mission-002");
  });

  it("resolveMissionId falls back to newest mission when none are running", async () => {
    const missions = buildMissions(
      mockMissionStore([
        makeMission("mission-001", "draft", "2026-01-01T00:00:00.000Z"),
        makeMission("mission-002", "completed", "2026-01-02T00:00:00.000Z"),
      ]),
      mockFeatureStore("mission-002"),
      mockAssertionStore("mission-002"),
      mockCheckpointStore("mission-002"),
    );

    await expect(missions.resolveMissionId()).resolves.toBe("mission-002");
  });

  it("resolveMissionId does not rely on store list ordering for fallback", async () => {
    const older = makeMission("mission-older", "draft", "2026-01-01T00:00:00.000Z");
    const newest = makeMission("mission-newest", "completed", "2026-01-03T00:00:00.000Z");
    const missionStore = {
      ...mockMissionStore([older, newest]),
      list: async () => [older, newest],
    };
    const missions = buildMissions(
      missionStore,
      mockFeatureStore(newest.id),
      mockAssertionStore(newest.id),
      mockCheckpointStore(newest.id),
    );

    await expect(missions.resolveMissionId()).resolves.toBe("mission-newest");
  });

  it("loadByMilestone returns feature and assertion slices for one milestone", async () => {
    const mission = makeMission("mission-001");
    const missions = buildMissions(
      mockMissionStore([mission]),
      mockFeatureStore(mission.id, [
        makeFeature("f1", "pending", "m1"),
        makeFeature("f2", "pending", "m2"),
      ]),
      mockAssertionStore(mission.id, [
        makeAssertion("a-f1", "f1", "m1"),
        makeAssertion("a-f2", "f2", "m2"),
      ]),
      mockCheckpointStore(mission.id),
    );

    await expect(missions.loadByMilestone(mission.id, "m1")).resolves.toEqual({
      features: [makeFeature("f1", "pending", "m1")],
      assertions: [makeAssertion("a-f1", "f1", "m1")],
    });
  });

  it("mockMissions isolates read slices by mission id", async () => {
    const missionOne = makeMission("mission-001");
    const missionTwo = makeMission("mission-002");
    const featureOne = { ...makeFeature("f1", "pending"), missionId: missionOne.id };
    const featureTwo = { ...makeFeature("f2", "pending"), missionId: missionTwo.id };
    const assertionOne = { ...makeAssertion("a-f1", "f1"), missionId: missionOne.id };
    const assertionTwo = { ...makeAssertion("a-f2", "f2"), missionId: missionTwo.id };
    const checkpointOne = { ...makeCheckpoint("checkpoint-1"), missionId: missionOne.id };
    const checkpointTwo = { ...makeCheckpoint("checkpoint-2"), missionId: missionTwo.id };
    const missions = mockMissions({
      missions: [missionOne, missionTwo],
      features: [featureOne, featureTwo],
      assertions: [assertionOne, assertionTwo],
      checkpoints: [checkpointOne, checkpointTwo],
    });

    await expect(missions.loadFullState(missionTwo.id)).resolves.toMatchObject({
      mission: missionTwo,
      features: [featureTwo],
      assertions: [assertionTwo],
      checkpoints: [checkpointTwo],
    });
  });

  it("resolveSingleActionableContext returns the unique active mission feature", async () => {
    const mission = makeMission("mission-001", "executing");
    const feature = makeFeature("f1", "in-progress", "m1");
    const assertion = makeAssertion("a-f1", "f1", "m1");
    const missions = buildMissions(
      mockMissionStore([mission]),
      mockFeatureStore(mission.id, [
        feature,
        makeFeature("f2", "done", "m1"),
        makeFeature("f3", "blocked", "m2"),
      ]),
      mockAssertionStore(mission.id, [
        assertion,
        makeAssertion("a-f2", "f2", "m1"),
      ]),
      mockCheckpointStore(mission.id),
    );

    await expect(missions.resolveSingleActionableContext()).resolves.toEqual({
      mission,
      milestone: mission.milestones[0],
      feature,
      assertions: [assertion],
    });
  });

  it("resolveSingleActionableContext returns undefined for ambiguous or empty contexts", async () => {
    const ambiguousMission = makeMission("mission-001", "executing");
    const ambiguous = buildMissions(
      mockMissionStore([ambiguousMission]),
      mockFeatureStore(ambiguousMission.id, [
        makeFeature("f1", "pending"),
        makeFeature("f2", "review"),
      ]),
      mockAssertionStore(ambiguousMission.id),
      mockCheckpointStore(ambiguousMission.id),
    );
    await expect(ambiguous.resolveSingleActionableContext()).resolves.toBeUndefined();

    const empty = buildMissions(
      mockMissionStore(),
      mockFeatureStore("empty"),
      mockAssertionStore("empty"),
      mockCheckpointStore("empty"),
    );
    await expect(empty.resolveSingleActionableContext()).resolves.toBeUndefined();
  });
});
