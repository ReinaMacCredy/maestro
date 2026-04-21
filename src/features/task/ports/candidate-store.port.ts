import type { TaskCandidate } from "../domain/task-candidate.js";

export interface CreateCandidateInput {
  readonly id: string;
  readonly sourceTaskId: string;
  readonly title: string;
  readonly reason: string;
  readonly keywords: readonly string[];
}

export interface CandidateStorePort {
  /** Persist a new candidate. Returns the stored candidate. */
  create(input: CreateCandidateInput): Promise<TaskCandidate>;

  /** Return all candidates in the store, unordered. Callers sort/filter. */
  all(): Promise<readonly TaskCandidate[]>;

  /** Remove a candidate by id. Tolerates missing entries. */
  delete(id: string): Promise<void>;
}
