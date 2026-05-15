import type {
  Principle,
  CreatePrincipleInput,
  PrincipleOutcomeRecord,
  MilestoneProfile,
} from "../domain/types.js";

export interface PrincipleStorePort {
  /** Return all principles in the store. */
  list(): Promise<readonly Principle[]>;

  /** Return principles that apply to a given milestone profile. */
  listByProfile(profile: MilestoneProfile): Promise<readonly Principle[]>;

  /** Get a single principle by id. Returns undefined if not found. */
  get(id: string): Promise<Principle | undefined>;

  /** Create a new principle. Throws if id already exists. */
  create(input: CreatePrincipleInput): Promise<Principle>;

  /** Remove a principle by id. Returns true if removed, false if not found. */
  remove(id: string): Promise<boolean>;

  /**
   * Append a principle outcome record to `outcomes.jsonl`. Best-effort:
   * implementations return false when persistence fails so callers can
   * decide whether to retry.
   */
  recordOutcome(record: PrincipleOutcomeRecord): Promise<boolean>;

  /**
   * List recorded principle outcomes. Tail-capped by default to keep
   * large outcome logs from blowing up memory. Malformed lines are
   * silently skipped.
   */
  listOutcomes(limit?: number): Promise<readonly PrincipleOutcomeRecord[]>;

  /**
   * Convenience filter: return the most recent `pending` rows for a given
   * handoff so the reply ingest usecase can resolve them to helpful or
   * unhelpful in one pass.
   */
  listPendingOutcomesForHandoff(
    handoffId: string,
  ): Promise<readonly PrincipleOutcomeRecord[]>;
}
