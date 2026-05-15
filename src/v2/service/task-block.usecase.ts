import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TaskBlockDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskBlockInput {
  readonly id: TaskId;
  readonly reason: string;
}

export async function taskBlock(deps: TaskBlockDeps, input: TaskBlockInput): Promise<Task> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);
  assertTaskTransition(existing.state, "blocked");
  const updated = await deps.taskStore.update(input.id, {
    state: "blocked",
    block_reason: input.reason,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      observabilityStore: deps.observabilityStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      task_id: existing.id,
      from_state: existing.state,
      to_state: "blocked",
      trigger_verb: "task:block",
      reason: input.reason,
    },
  );
  return updated;
}
