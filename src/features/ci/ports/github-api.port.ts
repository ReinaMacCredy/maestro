export interface CheckRunInput {
  readonly repository: string;
  readonly headSha: string;
  readonly name: string;
  readonly conclusion: "success" | "failure" | "action_required" | "neutral";
  readonly title: string;
  readonly summary: string;
}

export interface CheckRunRef {
  readonly id: number;
}

export interface GithubApiPort {
  readonly postCheckRun: (input: CheckRunInput) => Promise<CheckRunRef>;
  readonly patchCheckRun: (input: CheckRunInput & { readonly checkRunId: number }) => Promise<void>;
}
