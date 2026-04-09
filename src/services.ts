import type { GitPort } from "./ports/git.port.js";
import type { ConfigPort } from "./ports/config.port.js";
import type { SessionDetectPort } from "./features/session/ports/session-detect.port.js";
import type { NotesStorePort } from "./features/notes/ports/notes-store.port.js";
import type {
  MissionStorePort,
  FeatureStorePort,
  AssertionStorePort,
  CheckpointStorePort,
} from "./features/mission";
import type { CorrectionStorePort } from "./features/memory/ports/correction-store.port.js";
import type { LearningStorePort } from "./features/memory/ports/learning-store.port.js";
import type { RatchetStorePort } from "./features/ratchet/ports/ratchet-store.port.js";
import type { ProjectGraphStorePort } from "./features/graph/ports/project-graph-store.port.js";
import type { HandoffStorePort } from "./features/handoff/ports/handoff-store.port.js";
import { ShellGitAdapter } from "./adapters/git.adapter.js";
import { YamlConfigAdapter } from "./adapters/config.adapter.js";
import { buildMissionServices } from "./features/mission/services.js";
import { buildSessionServices } from "./features/session/services.js";
import { buildNotesServices } from "./features/notes/services.js";
import { buildRatchetServices } from "./features/ratchet/services.js";
import { buildHandoffServices } from "./features/handoff/services.js";
import { buildGraphServices } from "./features/graph/services.js";
import { buildMemoryServices } from "./features/memory/services.js";

export interface Services {
  readonly git: GitPort;
  readonly config: ConfigPort;
  readonly sessionDetect: SessionDetectPort;
  readonly notesStore: NotesStorePort;
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly checkpointStore: CheckpointStorePort;
  readonly correctionStore: CorrectionStorePort;
  readonly learningStore: LearningStorePort;
  readonly ratchetStore: RatchetStorePort;
  readonly projectGraphStore: ProjectGraphStorePort;
  readonly handoffStore: HandoffStorePort;
}

let instance: Services | undefined;

export function initServices(projectDir: string): Services {
  instance = {
    git: new ShellGitAdapter(),
    config: new YamlConfigAdapter(),
    ...buildMissionServices(projectDir),
    ...buildSessionServices(),
    ...buildNotesServices(projectDir),
    ...buildRatchetServices(projectDir),
    ...buildHandoffServices(projectDir),
    ...buildGraphServices(),
    ...buildMemoryServices(projectDir),
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
