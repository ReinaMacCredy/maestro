export type ContractAnchorFallback = "direct" | "reflog" | "merge-base" | "lost";

export interface GitTouchedFilesResult {
  readonly gitAvailable: boolean;
  readonly actualFilesTouched: readonly string[];
  readonly closedAtCommit?: string;
  readonly anchorFallback?: ContractAnchorFallback;
  readonly notes?: string;
}

export interface GitAnchorPort {
  resolveRepoRoot(cwd: string): Promise<string>;
  resolveHeadCommit(cwd: string): Promise<string | undefined>;
  collectTouchedFiles(input: {
    readonly repoRoot: string;
    readonly claimedAtCommit?: string;
    readonly rebaseFallback: "best-effort" | "fail";
  }): Promise<GitTouchedFilesResult>;
}
