// Composition root for v2 services. Producers can override individual ports
// for tests by passing a partial overrides bag.

import { FsSpecStore } from "../repo/fs-spec-store.adapter.js";
import type { SpecStorePort } from "../repo/spec-store.port.js";
import { JsonlEvidenceStore } from "../repo/jsonl-evidence-store.adapter.js";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import { JsonlTaskStore } from "../repo/jsonl-task-store.adapter.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { YamlArchitectureRules } from "../repo/yaml-architecture-rules.adapter.js";
import type { ArchitectureRulesPort } from "../repo/architecture-rules.port.js";
import { JsonlMissionStore } from "../repo/jsonl-mission-store.adapter.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import { FsPrinciplesStore } from "../repo/fs-principles-store.adapter.js";
import type { PrinciplesStorePort } from "../repo/principles-store.port.js";
import { BunProcessRunner } from "../repo/bun-process-runner.adapter.js";
import type { ProcessRunnerPort } from "../repo/process-runner.port.js";
import { JsonlObservabilityAdapter } from "../repo/jsonl-observability.adapter.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import { GitWorktreeStore } from "../repo/git-worktree-store.adapter.js";
import type { WorktreeStorePort } from "../repo/worktree-store.port.js";
import { FsHandoffEmitter } from "../repo/fs-handoff-emitter.adapter.js";
import type { HandoffEmitterPort } from "../repo/handoff-emitter.port.js";
import { FsNowMdWriter } from "../repo/fs-now-md-writer.adapter.js";
import type { NowMdWriterPort } from "../repo/now-md-writer.port.js";
import { buildNowMd } from "../service/build-now-md.js";

export interface V2Services {
  readonly specStore: SpecStorePort;
  readonly taskStore: TaskStorePort;
  readonly missionStore: MissionStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly architectureRules: ArchitectureRulesPort;
  readonly principlesStore: PrinciplesStorePort;
  readonly processRunner: ProcessRunnerPort;
  readonly observabilityStore: ObservabilityPort;
  readonly worktreeStore: WorktreeStorePort;
  readonly handoffEmitter: HandoffEmitterPort;
  readonly nowMdWriter: NowMdWriterPort;
}

export interface BuildV2ServicesOptions {
  readonly repoRoot: string;
  readonly overrides?: Partial<V2Services>;
}

export function buildV2Services(options: BuildV2ServicesOptions): V2Services {
  const { repoRoot, overrides } = options;
  const processRunner = overrides?.processRunner ?? new BunProcessRunner();
  return {
    specStore: overrides?.specStore ?? new FsSpecStore({ repoRoot }),
    taskStore: overrides?.taskStore ?? new JsonlTaskStore({ repoRoot }),
    missionStore: overrides?.missionStore ?? new JsonlMissionStore({ repoRoot }),
    evidenceStore: overrides?.evidenceStore ?? new JsonlEvidenceStore({ repoRoot }),
    architectureRules: overrides?.architectureRules ?? new YamlArchitectureRules({ repoRoot }),
    principlesStore: overrides?.principlesStore ?? new FsPrinciplesStore({ repoRoot }),
    processRunner,
    observabilityStore: overrides?.observabilityStore ?? new JsonlObservabilityAdapter({ repoRoot }),
    worktreeStore:
      overrides?.worktreeStore ?? new GitWorktreeStore({ repoRoot, processRunner }),
    handoffEmitter: overrides?.handoffEmitter ?? new FsHandoffEmitter({ repoRoot }),
    nowMdWriter:
      overrides?.nowMdWriter
      ?? new FsNowMdWriter({
        repoRoot,
        format: (tasks, now) => buildNowMd({ tasks, now }),
      }),
  };
}
