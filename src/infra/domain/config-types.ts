import type { AgentSlug } from "@/features/session";
import type { MemoryConfig } from "@/features/memory";
import type { WorkflowTemplate } from "@/features/mission";
import type { UiConfig } from "@/shared/domain/ui-config.js";

export interface MaestroConfig {
  readonly defaultAgent?: AgentSlug;
  readonly sourceRepo?: string;
  readonly contracts?: {
    readonly default?: "required" | "prompt" | "optional";
    readonly strict?: boolean;
    readonly overlapPolicy?: "fail" | "annotate";
    readonly rebaseFallback?: "best-effort" | "fail";
    readonly defaultMaxFilesTouched?: number;
    readonly staleReclaimContractPolicy?: "inherit" | "block";
  };
  readonly sessionDetection?: {
    readonly enabled: boolean;
    readonly agents: readonly AgentSlug[];
    readonly staleMinutes?: number;
  };
  readonly defaultWorkflow?: string;
  readonly workflowTemplates?: Readonly<Record<string, WorkflowTemplate>>;
  readonly ui?: UiConfig;
  readonly memory?: MemoryConfig;
}

export const DEFAULT_CONFIG: MaestroConfig = {
  contracts: {
    default: "prompt",
    strict: false,
    overlapPolicy: "fail",
    rebaseFallback: "best-effort",
    staleReclaimContractPolicy: "inherit",
  },
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
  defaultWorkflow: "plan-implement",
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
