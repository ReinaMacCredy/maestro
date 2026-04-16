import type { Task, UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";
import {
  claimedTaskCannotBeReopened,
  taskAlreadyCompleted,
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

  return store.update(id, validated);
}
