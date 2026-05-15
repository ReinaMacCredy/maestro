import { MaestroError } from "@/shared/errors.js";
import type { ContractStorePort } from "../ports/contract-store.port.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import type { Task, TaskMutationInput } from "../domain/task-types.js";
import { taskNotFound } from "../domain/task-errors.js";
import { isTaskId } from "../domain/task-id.js";

export interface DeleteTaskFlowDeps {
  readonly taskStore: TaskStorePort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
  readonly contractStore: ContractStorePort;
}

export async function deleteTaskFlow(
  deps: DeleteTaskFlowDeps,
  taskId: string,
  opts: TaskMutationInput = {},
): Promise<Task> {
  if (!isTaskId(taskId)) {
    throw taskNotFound(taskId);
  }

  const existing = await deps.taskStore.get(taskId);
  if (!existing) {
    await deleteTaskArtifacts(deps, taskId);
    throw taskNotFound(taskId);
  }

  assertDeleteTaskOwnership(existing, opts);
  await deleteTaskArtifacts(deps, taskId, existing.contractId);
  const deleted = await deps.taskStore.delete(taskId);
  return deleted;
}

function assertDeleteTaskOwnership(
  task: Pick<Task, "id" | "assignee">,
  actor: TaskMutationInput,
): void {
  if (!task.assignee || actor.force) {
    return;
  }
  if (!actor.sessionId) {
    throw new MaestroError(
      `Task ${task.id} is claimed by ${task.assignee}; 'delete' requires the owner session or --force`,
      [
        `Run 'maestro task delete ${task.id} --session ${task.assignee}' from the owning session`,
        "Or pass '--force' for an explicit operator override",
      ],
    );
  }
  if (task.assignee !== actor.sessionId) {
    throw new MaestroError(
      `Task ${task.id} is claimed by ${task.assignee}; current session cannot 'delete' it`,
      [
        "Retry from the owning session or pass '--force' to override",
        "Use 'maestro task show <id>' to inspect current ownership",
      ],
    );
  }
}

async function deleteTaskArtifacts(
  deps: DeleteTaskFlowDeps,
  taskId: string,
  knownContractId?: string,
): Promise<void> {
  await Promise.all([
    deps.continuationStore.delete(taskId),
    deps.continuationHistory.delete(taskId),
  ]);

  const contractId = knownContractId ?? (await deps.contractStore.getByTaskId(taskId))?.id;
  if (!contractId) {
    return;
  }

  await deps.contractStore.delete(contractId, {
    taskId,
    status: "discarded",
    at: new Date().toISOString(),
    reason: "task_deleted",
  });
}
