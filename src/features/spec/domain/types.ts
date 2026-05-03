/**
 * Spec domain types.
 *
 * A Spec is a first-class artifact attached to a Mission. It captures what
 * success means (AcceptanceCriteria), what is explicitly out of scope
 * (NonGoals), and (at L7.1) observable runtime signals that the system emits
 * when the spec is satisfied.
 *
 * The ProofMap (L3.5) joins Evidence rows with AcceptanceCriteria by the
 * stable `AcceptanceCriterion.id`. That id must never change once written.
 */

export interface AcceptanceCriterion {
  readonly id: string;
  readonly text: string;
}

export interface NonGoal {
  readonly text: string;
}

/**
 * Placeholder — populated at L7.1.
 *
 * A RuntimeSignal is an observable event or metric the running system emits
 * that can be used to confirm a criterion is satisfied without manual
 * verification.
 */
// TODO: populated at L7.1
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
