import type { Task, TaskMutationInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateBlockIds } from "../domain/task-validators.js";

export async function blockTasks(
  store: TaskStorePort,
  id: string,
  blockedTaskIds: readonly string[],
  opts: TaskMutationInput = {},
): Promise<Task> {
  return store.block(id, validateBlockIds(blockedTaskIds), opts);
}

export async function unblockTasks(
  store: TaskStorePort,
  id: string,
  blockedTaskIds: readonly string[],
  opts: TaskMutationInput = {},
): Promise<Task> {
  return store.unblock(id, validateBlockIds(blockedTaskIds), opts);
}
