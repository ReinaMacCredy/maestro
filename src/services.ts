import type { GitPort } from "./ports/git.port.js";
import type { ConfigPort } from "./ports/config.port.js";
import type { HandoffStorePort } from "./ports/handoff-store.port.js";
import type { CassPort } from "./ports/cass.port.js";
import type { SessionDetectPort } from "./ports/session-detect.port.js";
import type { NotesStorePort } from "./ports/notes-store.port.js";
import type { MissionStorePort } from "./ports/mission-store.port.js";
import type { FeatureStorePort } from "./ports/feature-store.port.js";
import type { AssertionStorePort } from "./ports/assertion-store.port.js";
import type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
import { ShellGitAdapter } from "./adapters/git.adapter.js";
import { YamlConfigAdapter } from "./adapters/config.adapter.js";
import { FsHandoffStoreAdapter } from "./adapters/handoff-store.adapter.js";
import { ShellCassAdapter } from "./adapters/cass.adapter.js";
import { ClaudeSessionDetectAdapter } from "./adapters/session-detect.adapter.js";
import { FsNotesStoreAdapter } from "./adapters/notes-store.adapter.js";
import { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
import { FsFeatureStoreAdapter } from "./adapters/feature-store.adapter.js";
import { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
import { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";

export interface Services {
  readonly git: GitPort;
  readonly config: ConfigPort;
  readonly handoffStore: HandoffStorePort;
  readonly cass: CassPort;
  readonly sessionDetect: SessionDetectPort;
  readonly notesStore: NotesStorePort;
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
}

let instance: Services | undefined;

export function initServices(projectDir: string): Services {
  instance = {
    git: new ShellGitAdapter(),
    config: new YamlConfigAdapter(),
    handoffStore: new FsHandoffStoreAdapter(projectDir),
    cass: new ShellCassAdapter(),
    sessionDetect: new ClaudeSessionDetectAdapter(),
    notesStore: new FsNotesStoreAdapter(projectDir),
    missionStore: new FsMissionStoreAdapter(projectDir),
    featureStore: new FsFeatureStoreAdapter(projectDir),
    assertionStore: new FsAssertionStoreAdapter(projectDir),
    checkpointStore: new FsCheckpointStoreAdapter(projectDir),
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
