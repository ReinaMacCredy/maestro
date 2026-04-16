import type { Task } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateBlockIds } from "../domain/task-validators.js";

export async function blockTasks(
  store: TaskStorePort,
  id: string,
  blockedTaskIds: readonly string[],
): Promise<Task> {
  return store.block(id, validateBlockIds(blockedTaskIds));
}

export async function unblockTasks(
  store: TaskStorePort,
  id: string,
  blockedTaskIds: readonly string[],
): Promise<Task> {
  return store.unblock(id, validateBlockIds(blockedTaskIds));
}
