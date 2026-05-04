// AcceptanceCriterion.id is the join key for the ProofMap and must
// never change once written.
export interface AcceptanceCriterion {
  readonly id: string;
  readonly text: string;
}

export interface NonGoal {
  readonly text: string;
}

export interface RuntimeSignal {
  readonly kind: string;
  readonly source: string;
}

export interface Spec {
  readonly schema_version: 1;
  readonly mission_id: string;
  readonly acceptance_criteria: readonly AcceptanceCriterion[];
  readonly non_goals: readonly NonGoal[];
  /** Populated at L7.1. */
  readonly runtime_signals: readonly RuntimeSignal[];
  readonly created_at: string;
  readonly updated_at: string;
}
