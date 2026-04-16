import type { Task, UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";
import {
  claimedTaskCannotBeReopened,
  closeViaCloseCommand,
  taskAlreadyClosed,
  taskStatusRequiresClaim,
} from "../domain/task-errors.js";

/**
 * Update a task. Rejects `--status closed` (route through closeTask instead)
 * so the close reason always gets captured.
 */
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
  if (existing.status === "closed") {
    throw taskAlreadyClosed(id);
  }

  if (validated.status === "closed") {
    throw closeViaCloseCommand();
  }
  if (!existing.assignee && validated.status === "in_progress") {
    throw taskStatusRequiresClaim("in_progress");
  }
  if (existing.assignee && validated.status === "open") {
    throw claimedTaskCannotBeReopened(id);
  }

  return store.update(id, validated);
}
