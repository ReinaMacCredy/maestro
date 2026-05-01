import type { MissionStorePort } from "./ports/mission-store.port.js";
import type { FeatureStorePort } from "./feature/ports/feature-store.port.js";
import type { AssertionStorePort } from "./ports/assertion-store.port.js";
import type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
import type { PrincipleStorePort } from "./ports/principle-store.port.js";
import type { ReplyStorePort } from "./reply/ports/reply-store.port.js";
import { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";
import { JsonlPrincipleStoreAdapter } from "./adapters/principle-store.adapter.js";
import { FsReplyStoreAdapter } from "./reply/adapters/fs-reply-store.adapter.js";

export interface MissionServices {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
  readonly principleStore: PrincipleStorePort;
  readonly replyStore: ReplyStorePort;
}

export function buildMissionServices(projectDir: string): MissionServices {
  return {
    missionStore: new FsMissionStoreAdapter(projectDir),
    featureStore: new FsFeatureStoreAdapter(projectDir),
    assertionStore: new FsAssertionStoreAdapter(projectDir),
    checkpointStore: new FsCheckpointStoreAdapter(projectDir),
    principleStore: new JsonlPrincipleStoreAdapter(projectDir),
    replyStore: new FsReplyStoreAdapter(projectDir),
  };
}
