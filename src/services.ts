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
import type { RuntimeStorePort } from "./ports/runtime-store.port.js";
import type { RuntimeEventStorePort } from "./ports/runtime-event-store.port.js";
import type { ExecutionStorePort } from "./ports/execution-store.port.js";
import type { TransportPort } from "./ports/transport.port.js";
import type { CorrectionStorePort } from "./ports/correction-store.port.js";
import type { LearningStorePort } from "./ports/learning-store.port.js";
import type { RatchetStorePort } from "./ports/ratchet-store.port.js";
import type { ProjectGraphStorePort } from "./ports/project-graph-store.port.js";
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
import { FsRuntimeStoreAdapter } from "./adapters/runtime-store.adapter.js";
import { FsRuntimeEventStoreAdapter } from "./adapters/runtime-event-store.adapter.js";
import { FsExecutionStoreAdapter } from "./adapters/execution-store.adapter.js";
import { MultiTransportAdapter } from "./adapters/multi-transport.adapter.js";
import { FsCorrectionStoreAdapter } from "./adapters/correction-store.adapter.js";
import { FsLearningStoreAdapter } from "./adapters/learning-store.adapter.js";
import { FsRatchetStoreAdapter } from "./adapters/ratchet-store.adapter.js";
import { FsProjectGraphStoreAdapter } from "./adapters/project-graph-store.adapter.js";

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
  readonly runtimeStore: RuntimeStorePort;
  readonly runtimeEventStore: RuntimeEventStorePort;
  readonly executionStore: ExecutionStorePort;
  readonly transport: TransportPort;
  readonly correctionStore: CorrectionStorePort;
  readonly learningStore: LearningStorePort;
  readonly ratchetStore: RatchetStorePort;
  readonly projectGraphStore: ProjectGraphStorePort;
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
    runtimeStore: new FsRuntimeStoreAdapter(projectDir),
    runtimeEventStore: new FsRuntimeEventStoreAdapter(projectDir),
    executionStore: new FsExecutionStoreAdapter(projectDir),
    transport: new MultiTransportAdapter(),
    correctionStore: new FsCorrectionStoreAdapter(projectDir),
    learningStore: new FsLearningStoreAdapter(projectDir),
    ratchetStore: new FsRatchetStoreAdapter(projectDir),
    projectGraphStore: new FsProjectGraphStoreAdapter(),
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
