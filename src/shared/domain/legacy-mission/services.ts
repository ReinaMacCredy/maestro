import type { MissionStorePort } from "./ports/mission-store.port.js";
import type { FeatureStorePort } from "./ports/feature-store.port.js";
import type { AssertionStorePort } from "./ports/assertion-store.port.js";
import type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
import { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "./adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";
import { buildMissions, type Missions } from "./missions.js";

export interface LegacyMissionServices {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
  readonly missions: Missions;
}

export function buildLegacyMissionServices(projectDir: string): LegacyMissionServices {
  const missionStore = new FsMissionStoreAdapter(projectDir);
  const featureStore = new FsFeatureStoreAdapter(projectDir);
  const assertionStore = new FsAssertionStoreAdapter(projectDir);
  const checkpointStore = new FsCheckpointStoreAdapter(projectDir);
  const missions = buildMissions(missionStore, featureStore, assertionStore, checkpointStore);

  return {
    missionStore,
    featureStore,
    assertionStore,
    checkpointStore,
    missions,
  };
}
