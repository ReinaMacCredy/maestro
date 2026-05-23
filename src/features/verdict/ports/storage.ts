import type { Verdict } from "../domain/types.js";

export interface ReadLatestWithCorruptionResult {
  readonly verdict: Verdict | undefined;
  readonly corruptCount: number;
}

export interface VerdictStorePort {
  write(taskId: string, verdict: Verdict): Promise<void>;
  readLatest(taskId: string): Promise<Verdict | undefined>;
  readVersion(taskId: string, verdictId: string): Promise<Verdict | undefined>;
  history(taskId: string): Promise<readonly Verdict[]>;
  findByTreeSha(treeSha: string): Promise<readonly Verdict[]>;
  // Returns the latest verdict for the task plus a count of files in the
  // task's verdict directory that were skipped because they could not be
  // parsed. Lets the status report surface corruption without forcing
  // `readLatest` to throw (which would poison other callers).
  readLatestWithCorruption(taskId: string): Promise<ReadLatestWithCorruptionResult>;
}
