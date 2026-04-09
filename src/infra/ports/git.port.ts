import type { GitState } from "@/infra/domain/git-types.js";

export interface GitPort {
  getState(cwd: string): Promise<GitState>;
  isRepo(cwd: string): Promise<boolean>;
}
