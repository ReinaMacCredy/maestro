export {
  scanDocGardening,
  type DocGardeningArgs,
  type DocGardeningDeps,
  type DocGardeningResult,
  type StaleReference,
  type StaleReferenceKind,
} from "./usecases/doc-gardening.usecase.js";
export {
  scanSlopCleanup,
  formatSlopCleanupLines,
  type SlopCleanupArgs,
  type SlopCleanupResult,
  type SlopFileGroup,
} from "./usecases/slop-cleanup.usecase.js";
export {
  regenPlan,
  formatPlanRegenLines,
  type PlanRegenArgs,
  type PlanRegenDeps,
  type PlanRegenResult,
  type PlanDrift,
  type PlanDriftKind,
} from "./usecases/plan-regen.usecase.js";
export { registerGcCommand } from "./commands/gc.command.js";
