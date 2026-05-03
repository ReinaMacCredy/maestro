export type {
  Spec,
  AcceptanceCriterion,
  NonGoal,
  RuntimeSignal,
} from "./domain/types.js";
export {
  CRITERION_ID_PATTERN,
  generateCriterionId,
  isCriterionId,
} from "./domain/spec-id.js";
export type { SpecStorePort } from "./ports/storage.js";
export { FsSpecStoreAdapter } from "./adapters/fs-spec-store.adapter.js";
export { createSpec } from "./usecases/create-spec.usecase.js";
export type { CreateSpecInput } from "./usecases/create-spec.usecase.js";
export { updateSpec } from "./usecases/update-spec.usecase.js";
export type { UpdateSpecInput } from "./usecases/update-spec.usecase.js";
export { getSpec } from "./usecases/get-spec.usecase.js";
export { listAcceptanceCriteria } from "./usecases/list-acceptance-criteria.usecase.js";
export { registerSpecCommand } from "./commands/spec.command.js";
export { buildSpecServices } from "./services.js";
export type { SpecServices } from "./services.js";
