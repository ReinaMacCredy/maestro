import type { Task, UpdateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";

export async function updateTask(
  store: TaskStorePort,
  id: string,
  patch: UpdateTaskInput,
): Promise<Task> {
  const validated = validateUpdateInput(patch);
  return store.update(id, validated);
}
