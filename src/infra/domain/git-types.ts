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
