// Composition root for v2 services. Producers can override individual ports
// for tests by passing a partial overrides bag.

import {
  FsSpecStore,
  type SpecStorePort,
} from "../repo/fs-spec-store.adapter.js";
import {
  JsonlEvidenceStore,
  type EvidenceStorePort,
} from "../repo/jsonl-evidence-store.adapter.js";
import {
  JsonlTaskStore,
  type TaskStorePort,
} from "../repo/jsonl-task-store.adapter.js";
import {
  YamlArchitectureRules,
  type ArchitectureRulesPort,
} from "../repo/yaml-architecture-rules.adapter.js";
import {
  JsonlExecPlanStore,
  type ExecPlanStorePort,
} from "../repo/jsonl-exec-plan-store.adapter.js";
import {
  FsPrinciplesStore,
  type PrinciplesStorePort,
} from "../repo/fs-principles-store.adapter.js";
import {
  BunProcessRunner,
  type ProcessRunnerPort,
} from "../repo/bun-process-runner.adapter.js";
import {
  JsonlObservabilityAdapter,
  type ObservabilityPort,
} from "../repo/jsonl-observability.adapter.js";

export interface V2Services {
  readonly specStore: SpecStorePort;
  readonly taskStore: TaskStorePort;
  readonly planStore: ExecPlanStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly architectureRules: ArchitectureRulesPort;
  readonly principlesStore: PrinciplesStorePort;
  readonly processRunner: ProcessRunnerPort;
  readonly observabilityStore: ObservabilityPort;
}

export interface BuildV2ServicesOptions {
  readonly repoRoot: string;
  readonly overrides?: Partial<V2Services>;
}

export function buildV2Services(options: BuildV2ServicesOptions): V2Services {
  const { repoRoot, overrides } = options;
  return {
    specStore: overrides?.specStore ?? new FsSpecStore({ repoRoot }),
    taskStore: overrides?.taskStore ?? new JsonlTaskStore({ repoRoot }),
    planStore: overrides?.planStore ?? new JsonlExecPlanStore({ repoRoot }),
    evidenceStore: overrides?.evidenceStore ?? new JsonlEvidenceStore({ repoRoot }),
    architectureRules: overrides?.architectureRules ?? new YamlArchitectureRules({ repoRoot }),
    principlesStore: overrides?.principlesStore ?? new FsPrinciplesStore({ repoRoot }),
    processRunner: overrides?.processRunner ?? new BunProcessRunner(),
    observabilityStore: overrides?.observabilityStore ?? new JsonlObservabilityAdapter({ repoRoot }),
  };
}
