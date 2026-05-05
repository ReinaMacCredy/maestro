// AcceptanceCriterion.id is the join key for the ProofMap and must
// never change once written.
export interface AcceptanceCriterion {
  readonly id: string;
  readonly text: string;
}

export interface NonGoal {
  readonly text: string;
}

export type RuntimeSignalOperator = ">" | "<" | ">=" | "<=" | "==";
export type RuntimeSignalSeverity = "info" | "warn" | "critical";

export interface RuntimeSignalThreshold {
  readonly operator: RuntimeSignalOperator;
  readonly value: number;
}

export interface RuntimeSignal {
  readonly name: string;
  readonly description?: string;
  readonly provider: string;
  readonly query: string;
  readonly threshold: RuntimeSignalThreshold;
  readonly severity: RuntimeSignalSeverity;
}

export interface CanaryStage {
  readonly percent: number;
  readonly hold_minutes: number;
}

export interface CanaryPlan {
  readonly stages: readonly CanaryStage[];
}

export interface RolloutPlan {
  readonly feature_flag?: string;
  readonly canary?: CanaryPlan;
  readonly rollback_command?: string;
}

export interface Spec {
  readonly schema_version: 2;
  readonly mission_id: string;
  readonly acceptance_criteria: readonly AcceptanceCriterion[];
  readonly non_goals: readonly NonGoal[];
  readonly runtime_signals: readonly RuntimeSignal[];
  readonly rollout_plan?: RolloutPlan;
  readonly created_at: string;
  readonly updated_at: string;
}
