export type {
  Spec,
  AcceptanceCriterion,
  NonGoal,
  RuntimeSignal,
  RuntimeSignalOperator,
  RuntimeSignalSeverity,
  RuntimeSignalThreshold,
  CanaryStage,
  CanaryPlan,
  RolloutPlan,
} from "./types.js";
export type { LegacySpecStorePort } from "./spec-store.port.js";
export { scoreSpec } from "./score-spec.js";
export type { SpecScoreResult } from "./score-spec.js";
export { FsSpecStoreAdapter, coerceSpec } from "./fs-spec-store.adapter.js";
