import type { Task } from "../domain/task-types.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import { taskNotFound } from "../domain/task-errors.js";

export async function showTask(
  store: TaskQueryPort,
  id: string,
): Promise<Task> {
  const task = await store.get(id);
  if (!task) {
    throw taskNotFound(id);
  }
  return task;
}
