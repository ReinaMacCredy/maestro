import type { AgentSlug } from "@/features/session";
import type {
  ExecutionConfig,
  WorkerConfig,
  SupervisionConfig,
  ParallelConfig,
} from "@/features/worker";
import type { MemoryConfig } from "@/features/memory";
import type { WorkflowTemplate } from "@/features/mission";
import type { UiConfig } from "@/shared/domain/ui-config.js";

export interface MaestroConfig {
  readonly defaultAgent?: AgentSlug;
  readonly sourceRepo?: string;
  readonly sessionDetection?: {
    readonly enabled: boolean;
    readonly agents: readonly AgentSlug[];
    readonly staleMinutes?: number;
  };
  readonly defaultWorkflow?: string;
  readonly workflowTemplates?: Readonly<Record<string, WorkflowTemplate>>;
  readonly execution?: ExecutionConfig;
  readonly workers?: Readonly<Record<string, WorkerConfig>>;
  readonly supervision?: SupervisionConfig;
  readonly parallel?: ParallelConfig;
  readonly ui?: UiConfig;
  readonly memory?: MemoryConfig;
}

export const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
  defaultWorkflow: "plan-implement",
  execution: {
    defaultWorker: "codex",
    stopOnFailure: true,
    retryBudget: 1,
    rotateWorkerOnRetry: false,
  },
  workers: {
    "claude-code": {
      enabled: true,
      transport: "cli",
      command: "claude",
      args: ["--print"],
      outputMode: "stream-json",
    },
    codex: {
      enabled: true,
      transport: "cli",
      command: "codex",
      args: [],
      outputMode: "raw",
    },
    // [WIP] Gemini worker -- registered but not integration-tested; disabled by default
    gemini: {
      enabled: false,
      transport: "cli",
      command: "gemini",
      args: [],
      outputMode: "stream-json",
    },
  },
  supervision: {
    level: "mid",
    staleAfterMs: 300_000,
    killGraceMs: 5_000,
    progressIntervalMs: 30_000,
  },
  // [WIP] Parallel execution -- config/UI scaffolding only; runtime always runs sequentially
  parallel: {
    enabled: false,
    maxConcurrent: 1,
  },
  ui: {
    missionControl: {
      backgroundMode: "solid",
    },
  },
  memory: {
    enabled: true,
    corrections: { enabled: true, matching: "keyword", auto_capture: "prompt", severity_default: "soft" },
    learnings: { enabled: true, compile_threshold: 5, max_age_days: 7 },
    ratchet: { enabled: false, enforcement: "warn" },
    graph: { enabled: true },
  } satisfies MemoryConfig,
};
