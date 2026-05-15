// Spec domain types and scoreSpec are now canonical in @/shared/domain/legacy-spec.
// These re-exports keep existing @/features/spec imports compiling during transition.
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
} from "@/shared/domain/legacy-spec/index.js";
export { scoreSpec } from "@/shared/domain/legacy-spec/index.js";
export type { SpecScoreResult } from "@/shared/domain/legacy-spec/index.js";
// LegacySpecStorePort is the v2-stable name; re-export as SpecStorePort for back-compat.
export type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";
export {
  CRITERION_ID_PATTERN,
  generateCriterionId,
  isCriterionId,
} from "./domain/spec-id.js";
export { FsSpecStoreAdapter } from "./adapters/fs-spec-store.adapter.js";
export { createSpec } from "./usecases/create-spec.usecase.js";
export type { CreateSpecInput } from "./usecases/create-spec.usecase.js";
export { updateSpec } from "./usecases/update-spec.usecase.js";
export type { UpdateSpecInput } from "./usecases/update-spec.usecase.js";
export { getSpec } from "./usecases/get-spec.usecase.js";
export { registerSpecCommand } from "./commands/spec.command.js";
export { buildSpecServices } from "./services.js";
export type { SpecServices } from "./services.js";
