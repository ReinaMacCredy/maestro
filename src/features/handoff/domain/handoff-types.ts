export const DEFAULT_HANDOFF_MODELS = {
  codex: "gpt-5.4",
  claude: "opus",
  hermes: "default",
} as const;

export type HandoffAgent = keyof typeof DEFAULT_HANDOFF_MODELS;

export interface HandoffRefs {
  readonly missionId?: string;
  readonly featureId?: string;
  readonly milestoneId?: string;
  readonly taskId?: string;
  readonly projectRoot?: string;
}

export interface HandoffRelevantFile {
  readonly path: string;
  readonly reason: string;
}

export interface HandoffPromptContext {
  readonly task: string;
  readonly context: readonly string[];
  readonly relevantFiles: readonly HandoffRelevantFile[];
  readonly currentState: readonly string[];
  readonly whatWasTried: readonly string[];
  readonly decisions: readonly string[];
  readonly acceptanceCriteria: readonly string[];
  readonly constraints: readonly string[];
  readonly refs: HandoffRefs;
}

export interface HandoffWorktree {
  readonly slug: string;
  readonly baseBranch: string;
  readonly branch: string;
  readonly path: string;
}

export type HandoffStatus =
  | "launching"
  | "launched"
  | "completed"
  | "failed"
  | "consumed";

export interface HandoffRecord {
  readonly id: string;
  readonly createdAt: string;
  readonly task: string;
  readonly name: string;
  readonly agent: HandoffAgent;
  readonly model: string;
  readonly status: HandoffStatus;
  readonly wait: boolean;
  readonly sourceDir: string;
  readonly targetDir: string;
  readonly promptPath: string;
  readonly outputPath: string;
  readonly command: readonly string[];
  readonly refs: HandoffRefs;
  readonly createdByAgent?: string;
  readonly createdBySessionId?: string;
  readonly pickedUpByAgent?: string;
  readonly pickedUpBySessionId?: string;
  readonly pickedUpAt?: string;
  readonly consumedAt?: string;
  readonly worktree?: HandoffWorktree;
  readonly pid?: number;
  readonly exitCode?: number;
  readonly errorMessage?: string;
}

export interface HandoffLaunchRequest {
  readonly prompt: string;
  readonly targetDir: string;
  readonly model: string;
  readonly modelProvided: boolean;
  readonly name: string;
  readonly wait: boolean;
  readonly logPath: string;
  readonly env?: Record<string, string>;
}

export interface HandoffLaunchResult {
  readonly command: readonly string[];
  readonly pid?: number;
  readonly exitCode?: number;
}

export interface HandoffLaunchPort {
  readonly agent: HandoffAgent;
  launch(request: HandoffLaunchRequest): Promise<HandoffLaunchResult>;
}

export interface HandoffStorePort {
  create(input: {
    readonly task: string;
    readonly name: string;
    readonly agent: HandoffAgent;
    readonly model: string;
    readonly wait: boolean;
    readonly sourceDir: string;
    readonly targetDir: string;
    readonly refs: HandoffRefs;
    readonly createdByAgent?: string;
    readonly createdBySessionId?: string;
    readonly worktree?: HandoffWorktree;
    readonly prompt: string;
  }): Promise<HandoffRecord>;
  update(record: HandoffRecord): Promise<HandoffRecord>;
  consume(input: {
    readonly id: string;
    readonly agent: string;
    readonly sessionId?: string;
    readonly pickedUpAt: string;
  }): Promise<HandoffRecord>;
  get(id: string): Promise<HandoffRecord | undefined>;
  list(): Promise<readonly HandoffRecord[]>;
  listOpenForTask(input: {
    readonly taskId: string;
    readonly projectRoot: string;
  }): Promise<readonly HandoffRecord[]>;
  resolveArtifactPath(relativePath: string): string;
}
