export {
  inspectRun,
  formatInspectRunLines,
  type InspectRunArgs,
  type InspectRunDeps,
  type InspectRunResult,
  type RunArtifact,
} from "./usecases/inspect-run.usecase.js";
export {
  inspectTokenBudget,
  formatTokenBudgetLines,
  type TokenBudgetResult,
  type TokenBudgetRow,
} from "./usecases/inspect-token-budget.usecase.js";
export { registerInspectCommand } from "./commands/inspect.command.js";
