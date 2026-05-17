export type { PlanInput, PlanCheckFinding, PlanCheckResult } from "./domain/types.js";
export { AgentReportSchema } from "./domain/legacy-report-schema.js";
export type { AgentReport } from "./domain/legacy-report-schema.js";
export { checkPlan } from "./usecases/check-plan.js";
export type { CheckPlanInput } from "./usecases/check-plan.js";
export { buildPlanServices } from "./services.js";
export type { PlanServices } from "./services.js";
export { registerPlanCheckCommand } from "./commands/plan-check.command.js";
