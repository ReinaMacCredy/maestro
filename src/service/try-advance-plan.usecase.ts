import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { ExecPlanStorePort } from "../repo/exec-plan-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import type { ExecPlan } from "../types/exec-plan.js";
import { isTerminalTaskState } from "../types/task-state.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TryAdvancePlanDeps {
  readonly planStore: ExecPlanStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TryAdvancePlanInput {
  readonly plan_id?: string;
  readonly trigger_task_verb: "task:claim" | "task:ship" | "task:abandon";
}

// ADR-0011: plans auto-advance off the back of task transitions. This helper
// is idempotent (a no-op for plans already past the target state) so individual
// task verbs can call it without caring about ordering or replay safety.
export async function tryAdvancePlan(
  deps: TryAdvancePlanDeps,
  input: TryAdvancePlanInput,
): Promise<ExecPlan | undefined> {
  if (!input.plan_id) return undefined;
  const plan = await deps.planStore.get(input.plan_id);
  if (!plan) return undefined;

  if (input.trigger_task_verb === "task:claim") {
    if (plan.state !== "planned") return plan;
    return advance(deps, plan, "in-progress", "plan:auto-start");
  }

  if (plan.state !== "in-progress" && plan.state !== "planned") return plan;
  const children = await deps.taskStore.listByPlanId(plan.id);
  if (children.length === 0) return plan;
  const allTerminal = children.every((t) => isTerminalTaskState(t.state));
  if (!allTerminal) return plan;
  // If we're still at 'planned' (e.g. every claimed task abandoned before
  // anyone advanced the plan), pass through in-progress so the state
  // machine stays well-formed.
  if (plan.state === "planned") {
    const stepped = await advance(deps, plan, "in-progress", "plan:auto-start");
    return advance(deps, stepped, "completed", "plan:auto-complete");
  }
  return advance(deps, plan, "completed", "plan:auto-complete");
}

async function advance(
  deps: TryAdvancePlanDeps,
  plan: ExecPlan,
  to: "in-progress" | "completed",
  trigger_verb: string,
): Promise<ExecPlan> {
  const updated = await deps.planStore.update(plan.id, { state: to });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      plan_id: plan.id,
      from_state: plan.state,
      to_state: to,
      trigger_verb,
    },
  );
  return updated;
}
