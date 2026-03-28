/** Supported agent identifiers. Extensible -- CASS connectors are the source of truth. */
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

export type HandoffStatus = "pending" | "picked-up" | "completed";

export type DetectionMethod = "pid" | "env" | "cwd-fallback" | "explicit";

export interface HandoffSession {
  readonly agent: AgentSlug;
  readonly sessionId: string;
  readonly sourcePath: string;
  readonly startedAt?: number;
  readonly detectionMethod?: DetectionMethod;
}

export interface PlanTask {
  readonly id: string;
  readonly description: string;
  readonly status: "pending" | "done" | "blocked";
  readonly dependsOn: readonly string[];
}

export interface HandoffPlan {
  readonly tasks: readonly PlanTask[];
  readonly completed: number;
  readonly remaining: number;
}

export interface GitState {
  readonly branch: string;
  readonly recentCommits: readonly string[];
  readonly changedFiles: readonly string[];
  readonly workingTreeClean: boolean;
  readonly diffStat: string;
}

export interface Handoff {
  readonly id: string;
  readonly timestamp: string;
  readonly message: string;
  readonly session: HandoffSession;
  readonly plan?: HandoffPlan;
  readonly sitrep: string;
  readonly quickstart: string;
  readonly instructions?: string;
  readonly git: GitState;
}

export interface HandoffEnvelope {
  readonly handoff: Handoff;
  readonly status: HandoffStatus;
  readonly pickedUpAt?: string;
  readonly pickedUpBy?: AgentSlug;
  readonly completedAt?: string;
  readonly report?: string;
}

export interface NoteEntry {
  readonly timestamp: string;
  readonly content: string;
  readonly git_branch: string;
}

export interface MaestroConfig {
  readonly defaultAgent?: AgentSlug;
  readonly sourceRepo?: string;
  readonly cassPath?: string;
  readonly handoffDir?: string;
  readonly promptTemplate?: string;
  readonly sessionDetection?: {
    readonly enabled: boolean;
    readonly agents: readonly AgentSlug[];
    readonly staleMinutes?: number;
  };
}

export interface CassSearchResult {
  readonly title: string;
  readonly snippet: string;
  readonly content: string;
  readonly score: number;
  readonly sourcePath: string;
  readonly agent: string;
  readonly lineNumber: number;
  readonly createdAt: number;
}

export interface CassSearchResponse {
  readonly query: string;
  readonly count: number;
  readonly totalMatches: number;
  readonly hits: readonly CassSearchResult[];
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
  readonly pendingHandoffs: readonly HandoffEnvelope[];
  readonly cassAvailable: boolean;
  readonly gitAvailable: boolean;
}
