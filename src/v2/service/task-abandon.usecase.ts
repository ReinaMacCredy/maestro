import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TaskAbandonDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskAbandonInput {
  readonly id: TaskId;
  readonly reason: string;
}

export async function taskAbandon(deps: TaskAbandonDeps, input: TaskAbandonInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  assertTaskTransition(existing.state, "abandoned");
  const updated = await deps.taskStore.update(input.id, {
    state: "abandoned",
    abandon_reason: input.reason,
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
      to_state: "abandoned",
      trigger_verb: "task:abandon",
      reason: input.reason,
    },
  );
  return updated;
}
