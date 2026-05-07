import type { Verdict } from "../domain/types.js";

export interface VerdictStorePort {
  write(taskId: string, verdict: Verdict): Promise<void>;
  readLatest(taskId: string): Promise<Verdict | undefined>;
  readVersion(taskId: string, verdictId: string): Promise<Verdict | undefined>;
  history(taskId: string): Promise<readonly Verdict[]>;
  findByTreeSha(treeSha: string): Promise<readonly Verdict[]>;
}
