export const DEFAULT_HANDOFF_MODELS = {
  codex: "gpt-5.4",
  claude: "opus",
} as const;

export type HandoffProvider = keyof typeof DEFAULT_HANDOFF_MODELS;

export interface HandoffRefs {
  readonly missionId?: string;
  readonly featureId?: string;
  readonly milestoneId?: string;
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

export type HandoffLaunchStatus =
  | "launching"
  | "launched"
  | "completed"
  | "failed";

export interface HandoffLaunchRecord {
  readonly id: string;
  readonly createdAt: string;
  readonly task: string;
  readonly name: string;
  readonly provider: HandoffProvider;
  readonly model: string;
  readonly status: HandoffLaunchStatus;
  readonly wait: boolean;
  readonly sourceDir: string;
  readonly targetDir: string;
  readonly promptPath: string;
  readonly outputPath: string;
  readonly command: readonly string[];
  readonly refs: HandoffRefs;
  readonly worktree?: HandoffWorktree;
  readonly pid?: number;
  readonly exitCode?: number;
  readonly errorMessage?: string;
}

export interface HandoffLaunchRequest {
  readonly prompt: string;
  readonly targetDir: string;
  readonly model: string;
  readonly name: string;
  readonly wait: boolean;
  readonly logPath: string;
}

export interface HandoffLaunchResult {
  readonly command: readonly string[];
  readonly pid?: number;
  readonly exitCode?: number;
}

export interface HandoffLaunchPort {
  readonly provider: HandoffProvider;
  launch(request: HandoffLaunchRequest): Promise<HandoffLaunchResult>;
}

export interface LaunchStorePort {
  create(input: {
    readonly task: string;
    readonly name: string;
    readonly provider: HandoffProvider;
    readonly model: string;
    readonly wait: boolean;
    readonly sourceDir: string;
    readonly targetDir: string;
    readonly refs: HandoffRefs;
    readonly worktree?: HandoffWorktree;
    readonly prompt: string;
  }): Promise<HandoffLaunchRecord>;
  update(record: HandoffLaunchRecord): Promise<HandoffLaunchRecord>;
  get(id: string): Promise<HandoffLaunchRecord | undefined>;
  list(): Promise<readonly HandoffLaunchRecord[]>;
  resolveArtifactPath(relativePath: string): string;
}
