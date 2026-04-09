/** Supported agent identifiers. */
export type AgentSlug =
  | "claude-code"
  | "codex"
  | "gemini"
  | "opencode"
  | "amp"
  | "cline"
  | "aider"
  | "cursor"
  | (string & {});

/**
 * Phase 1 strip: AgentSession replaces the old HandoffSession shape.
 * The conductor no longer owns handoff records; this type describes the
 * identity the session-detect adapter emits so memory + notes can
 * associate artifacts with the current shell.
 */
export interface AgentSession {
  readonly agent: AgentSlug;
  readonly sessionId: string;
  readonly sourcePath: string;
  readonly startedAt?: number;
}

export interface GitState {
  readonly branch: string;
  readonly recentCommits: readonly string[];
  readonly changedFiles: readonly string[];
  readonly fileChanges?: readonly GitFileChange[];
  readonly workingTreeClean: boolean;
  readonly diffStat: string;
}

export interface GitFileChange {
  readonly path: string;
  readonly kind:
    | "added"
    | "modified"
    | "deleted"
    | "renamed"
    | "copied"
    | "typechange"
    | "untracked"
    | "conflicted";
}

export interface NoteEntry {
  readonly timestamp: string;
  readonly content: string;
  readonly git_branch: string;
}

/** A single phase in a workflow template */
export interface WorkflowPhase {
  readonly kind: import("@/features/mission").MilestoneKind;
  readonly label: string;
  readonly profile?: import("@/features/mission").MilestoneProfile;
  readonly description?: string;
}

/** Named workflow template — a reusable milestone sequence */
export interface WorkflowTemplate {
  readonly description: string;
  readonly phases: readonly WorkflowPhase[];
}

export type MissionControlBackgroundMode = "solid" | "terminal";

export interface MissionControlUiConfig {
  readonly backgroundMode?: MissionControlBackgroundMode;
}

export interface UiConfig {
  readonly missionControl?: MissionControlUiConfig;
}

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
  readonly execution?: import("./worker-types.js").ExecutionConfig;
  readonly workers?: Readonly<Record<string, import("./worker-types.js").WorkerConfig>>;
  readonly supervision?: import("./worker-types.js").SupervisionConfig;
  readonly parallel?: import("./worker-types.js").ParallelConfig;
  readonly ui?: UiConfig;
  readonly memory?: import("@/features/memory").MemoryConfig;
}

export interface DoctorCheck {
  readonly name: string;
  readonly status: "ok" | "warn" | "fail";
  readonly message: string;
  readonly fix?: string;
}

export interface StatusReport {
  readonly initialized: boolean;
  readonly configSource: "global" | "project" | "none";
  /**
   * Phase 2 reintroduced real UKI handoff records here so CLI status
   * and Mission Control agree on pending work.
   */
  readonly pendingHandoffs: readonly import("./uki-types.js").UkiHandoff[];
  /**
   * Phase 1 strip: cassAvailable is still on the struct for the same
   * structural reason as pendingHandoffs. Value is always false until
   * the field is removed outright in a later phase.
   */
  readonly cassAvailable: boolean;
  readonly gitAvailable: boolean;
}
