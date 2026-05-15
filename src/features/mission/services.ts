import type { MissionStorePort } from "@/shared/domain/legacy-mission";
import type { FeatureStorePort } from "@/shared/domain/legacy-mission";
import type { AssertionStorePort } from "@/shared/domain/legacy-mission";
import type { CheckpointStorePort } from "@/shared/domain/legacy-mission";
import type { PrincipleStorePort } from "@/features/principle";
import type { ReplyStorePort } from "@/features/reply";
import { FsMissionStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsFeatureStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsAssertionStoreAdapter } from "@/shared/domain/legacy-mission";
import { FsCheckpointStoreAdapter } from "@/shared/domain/legacy-mission";
import { buildPrincipleServices } from "@/features/principle";
import { buildReplyServices } from "@/features/reply";
import { buildMissions, type Missions } from "@/shared/domain/legacy-mission";

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
