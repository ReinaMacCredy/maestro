import type { GitState, GitWorktree } from "@/infra/domain/git-types.js";

export interface GitPort {
  getState(cwd: string): Promise<GitState>;
  isRepo(cwd: string): Promise<boolean>;
  getCurrentBranch(cwd: string): Promise<string>;
  createWorktree(
    cwd: string,
    input: {
      readonly slug: string;
      readonly baseBranch: string;
      readonly branchPrefix: string;
    },
  ): Promise<GitWorktree>;
}
