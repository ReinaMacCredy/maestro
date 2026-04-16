import { indexTasksById, type Task, type UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";
import {
  claimedTaskCannotBeReopened,
  taskAlreadyCompleted,
  taskBlockedByOpenTasks,
  taskReasonRequiresCompletedStatus,
  taskStatusRequiresClaim,
} from "../domain/task-errors.js";

export async function updateTask(
  store: TaskStorePort,
  id: string,
  patch: UpdateTaskInput,
): Promise<Task> {
  const validated = validateUpdateInput(patch);
  const existing = await store.get(id);
  if (!existing) {
    return store.update(id, validated);
  }
  if (existing.status === "completed") {
    throw taskAlreadyCompleted(id);
  }

  if (validated.reason !== undefined && validated.status !== "completed") {
    throw taskReasonRequiresCompletedStatus();
  }
  if (!existing.assignee && validated.status === "in_progress") {
    throw taskStatusRequiresClaim("in_progress");
  }
  if (existing.assignee && validated.status === "pending") {
    throw claimedTaskCannotBeReopened(id);
  }
  if (validated.status === "in_progress" || validated.status === "completed") {
    const tasks = indexTasksById(await store.all());
    const blockers = existing.blockedBy.filter((blockerId) => {
      const blocker = tasks.get(blockerId);
      return blocker === undefined || blocker.status !== "completed";
    });
    if (blockers.length > 0) {
      throw taskBlockedByOpenTasks(id, blockers);
    }
  }

  return store.update(id, validated);
}
