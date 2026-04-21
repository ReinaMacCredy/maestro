import type { Task, TaskStorePort } from "../index.js";

export async function reopenTask(
  taskStore: TaskStorePort,
  id: string,
): Promise<Task> {
  return taskStore.reopen(id);
}
