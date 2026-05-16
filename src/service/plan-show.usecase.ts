import type { ExecPlanStorePort } from "../repo/exec-plan-store.port.js";
import { ExecPlanNotFoundError } from "../repo/exec-plan-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { ExecPlan, ExecPlanId } from "../types/exec-plan.js";
import type { Task } from "../types/task.js";

export interface PlanShowDeps {
  readonly planStore: ExecPlanStorePort;
  readonly taskStore: TaskStorePort;
}

export interface PlanShowResult {
  readonly plan: ExecPlan;
  readonly tasks: readonly Task[];
}

export async function planShow(deps: PlanShowDeps, id: ExecPlanId): Promise<PlanShowResult> {
  const plan = await deps.planStore.get(id);
  if (!plan) throw new ExecPlanNotFoundError(id);
  const tasks = await deps.taskStore.listByPlanId(id);
  return { plan, tasks };
}
