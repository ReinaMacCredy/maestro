import type { AgentSlug } from "@/shared/domain/agent-slug.js";
import type { WorkflowTemplate } from "@/features/mission";
import type { UiConfig } from "@/tui/shared/ui-config.js";

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
  readonly defaultWorkflow?: string;
  readonly workflowTemplates?: Readonly<Record<string, WorkflowTemplate>>;
  readonly ui?: UiConfig;
}

export const DEFAULT_CONFIG: MaestroConfig = {
  contracts: {
    default: "prompt",
    strict: false,
    overlapPolicy: "fail",
    rebaseFallback: "best-effort",
    staleReclaimContractPolicy: "inherit",
  },
  defaultWorkflow: "plan-implement",
  // No `ui` block: ui.missionControl.backgroundMode is global-only
  // (see GLOBAL_ONLY_CONFIG_KEYS in shared/domain/ui-config.ts). Writing
  // it as a project default would make `maestro doctor` flag every fresh
  // init's config.yaml as containing keys it will ignore. Runtime falls
  // back to "solid" via getMissionControlBackgroundMode.
};
