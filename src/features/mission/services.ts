import type { MissionStorePort } from "./ports/mission-store.port.js";
import type { FeatureStorePort } from "./feature/ports/feature-store.port.js";
import type { AssertionStorePort } from "./ports/assertion-store.port.js";
import type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
import type { PrincipleStorePort } from "@/features/principle";
import type { ReplyStorePort } from "@/features/reply";
import { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";
import { buildPrincipleServices } from "@/features/principle";
import { buildReplyServices } from "@/features/reply";
import { buildMissions, type Missions } from "./usecases/missions.usecase.js";

export interface MissionServices {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
  readonly principleStore: PrincipleStorePort;
  readonly replyStore: ReplyStorePort;
  readonly missions: Missions;
}

export function buildMissionServices(projectDir: string): MissionServices {
  const missionStore = new FsMissionStoreAdapter(projectDir);
  const featureStore = new FsFeatureStoreAdapter(projectDir);
  const assertionStore = new FsAssertionStoreAdapter(projectDir);
  const checkpointStore = new FsCheckpointStoreAdapter(projectDir);
  const { principleStore } = buildPrincipleServices(projectDir);
  const { replyStore } = buildReplyServices(projectDir);
  const missions = buildMissions(missionStore, featureStore, assertionStore, checkpointStore);

  return {
    missionStore,
    featureStore,
    assertionStore,
    checkpointStore,
    principleStore,
    replyStore,
    missions,
  };
}
