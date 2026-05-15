import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TaskShipDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskShipInput {
  readonly id: TaskId;
  readonly pr_url?: string;
}

export async function taskShip(deps: TaskShipDeps, input: TaskShipInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  assertTaskTransition(existing.state, "shipped");
  const merged_at = (deps.clock ?? (() => new Date()))().toISOString();
  const updated = await deps.taskStore.update(input.id, {
    state: "shipped",
    pr_url: input.pr_url,
    merged_at,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      task_id: existing.id,
      from_state: existing.state,
      to_state: "shipped",
      trigger_verb: "task:ship",
      verdict: "PASS",
    },
  );
  return updated;
}
