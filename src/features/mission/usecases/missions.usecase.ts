import type {
  Assertion,
  Checkpoint,
  Feature,
  Milestone,
  Mission,
} from "../domain/mission-types.js";
import { missionNotFound } from "../domain/errors.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { FeatureStorePort } from "../feature/ports/feature-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../ports/checkpoint-store.port.js";

export interface MissionFullState {
  readonly mission: Mission;
  readonly features: readonly Feature[];
  readonly assertions: readonly Assertion[];
  readonly checkpoints: readonly Checkpoint[];
}

export interface ActiveMissionContext {
  readonly mission: Mission;
  readonly milestone: Milestone;
  readonly feature: Feature;
  readonly assertions: readonly Assertion[];
}

export interface Missions {
  get(id: string): Promise<Mission | undefined>;
  resolveMissionId(explicit?: string): Promise<string | undefined>;
  loadFullState(id: string): Promise<MissionFullState>;
  loadByMilestone(id: string, milestoneId: string): Promise<{
    readonly features: readonly Feature[];
    readonly assertions: readonly Assertion[];
  }>;
  resolveSingleActionableContext(): Promise<ActiveMissionContext | undefined>;
}

export function buildMissions(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  checkpointStore: CheckpointStorePort,
): Missions {
  return {
    get: (id) => missionStore.get(id),

    async resolveMissionId(explicit) {
      if (explicit) return explicit;

      const missions = [...await missionStore.list()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
      if (missions.length === 0) return undefined;

      const active = missions.find((mission) => mission.status === "executing" || mission.status === "paused");
      if (active) return active.id;

      return missions[0]!.id;
    },

    async loadFullState(id) {
      const [mission, features, assertions, checkpoints] = await Promise.all([
        missionStore.get(id),
        featureStore.list(id),
        assertionStore.list(id),
        checkpointStore.list(id),
      ]);

      if (!mission) {
        throw missionNotFound(id);
      }

      return { mission, features, assertions, checkpoints };
    },

    async loadByMilestone(id, milestoneId) {
      const [features, assertions] = await Promise.all([
        featureStore.list(id, { milestoneId }),
        assertionStore.listByMilestone(id, milestoneId),
      ]);

      return { features, assertions };
    },

    async resolveSingleActionableContext() {
      const missions = await missionStore.list();
      const mission = missions.find((item) => item.status === "executing" || item.status === "paused")
        ?? (missions.length === 1 ? missions[0] : undefined);

      if (!mission) return undefined;

      const [features, allAssertions] = await Promise.all([
        featureStore.list(mission.id),
        assertionStore.list(mission.id),
      ]);
      const actionable = features.filter((feature) => feature.status !== "done" && feature.status !== "blocked");
      if (actionable.length !== 1) return undefined;

      const feature = actionable[0]!;
      const milestone = mission.milestones.find((item) => item.id === feature.milestoneId);
      if (!milestone) return undefined;

      const assertions = allAssertions.filter((item) => item.featureId === feature.id);
      return { mission, milestone, feature, assertions };
    },
  };
}
