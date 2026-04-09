import type { GitState } from "../domain/types.js";

export interface GitPort {
  getState(cwd: string): Promise<GitState>;
  isRepo(cwd: string): Promise<boolean>;
}
