import type { ContractStorePort } from "../ports/contract-store.port.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import type { Task } from "../domain/task-types.js";
import { taskNotFound } from "../domain/task-errors.js";

export interface DeleteTaskFlowDeps {
  readonly taskStore: TaskStorePort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
  readonly contractStore: ContractStorePort;
}

export async function deleteTaskFlow(
  deps: DeleteTaskFlowDeps,
  taskId: string,
): Promise<Task> {
  const existing = await deps.taskStore.get(taskId);
  if (!existing) {
    throw taskNotFound(taskId);
  }

  const deleted = await deps.taskStore.delete(taskId);
  await Promise.all([
    deps.continuationStore.delete(taskId),
    deps.continuationHistory.delete(taskId),
  ]);

  const contractId = existing.contractId ?? (await deps.contractStore.getByTaskId(taskId))?.id;
  if (contractId) {
    await deps.contractStore.delete(contractId, {
      taskId,
      status: "discarded",
      at: new Date().toISOString(),
      reason: "task_deleted",
    });
  }

  return deleted;
}
