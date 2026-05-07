import { checkPlan } from "./usecases/check-plan.js";
import type { CheckPlanInput } from "./usecases/check-plan.js";
import type { PlanCheckResult } from "./domain/types.js";

export interface PlanServices {
  readonly checkPlan: (input: CheckPlanInput) => PlanCheckResult;
}

export function buildPlanServices(): PlanServices {
  return { checkPlan };
}
