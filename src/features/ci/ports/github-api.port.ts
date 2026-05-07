export type CheckRunConclusion = "success" | "failure" | "action_required";

export interface CheckRunInput {
  readonly repository: string;
  readonly headSha: string;
  readonly name: string;
  readonly conclusion: CheckRunConclusion;
  readonly title: string;
  readonly summary: string;
}

export interface CheckRunRef {
  readonly id: number;
}

export interface TriggerAutoMergeInput {
  readonly repository: string;
  readonly pr: number;
  readonly mergeMethod?: "merge" | "squash" | "rebase";
}

export interface GithubApiPort {
  readonly getPullRequestAuthor: (input: { repository: string; pr: number }) => Promise<string>;
  readonly postCheckRun: (input: CheckRunInput) => Promise<CheckRunRef>;
  readonly patchCheckRun: (input: CheckRunInput & { readonly checkRunId: number }) => Promise<void>;
  readonly triggerAutoMerge: (input: TriggerAutoMergeInput) => Promise<void>;
  readonly listOpenPullRequests: (input: { repository: string }) => Promise<readonly number[]>;
  readonly getPullRequestFiles: (input: { repository: string; pr: number }) => Promise<readonly string[]>;
}
