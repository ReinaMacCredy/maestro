import type { GitState, GitWorktree } from "@/infra/domain/git-types.js";

export interface GitPort {
  readonly status?: (cwd: string) => Promise<GitState>;
  readonly worktrees?: (cwd: string) => Promise<readonly GitWorktree[]>;
}
