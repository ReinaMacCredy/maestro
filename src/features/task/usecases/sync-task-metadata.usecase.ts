import type { Task, TaskMetadataPatch } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function syncTaskMetadata(
  store: TaskStorePort,
  id: string,
  patch: TaskMetadataPatch,
): Promise<Task> {
  return store.syncMetadata(id, patch);
}
