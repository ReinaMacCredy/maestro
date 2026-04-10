import type { Task, CloseTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { taskAlreadyClosed, taskNotFound } from "../domain/task-errors.js";

export async function closeTask(
  store: TaskStorePort,
  id: string,
  input: CloseTaskInput,
): Promise<Task> {
  const existing = await store.get(id);
  if (!existing) {
    throw taskNotFound(id);
  }
  if (existing.status === "closed") {
    throw taskAlreadyClosed(id);
  }
  return store.close(id, input);
}
