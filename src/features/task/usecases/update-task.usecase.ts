import type { Task, UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";
import { closeViaCloseCommand } from "../domain/task-errors.js";

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

  if (validated.status === "closed") {
    throw closeViaCloseCommand();
  }

  return store.update(id, validated);
}
