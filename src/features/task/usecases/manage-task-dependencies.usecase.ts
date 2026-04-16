import type { Task } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateDependencyIds } from "../domain/task-validators.js";

export async function addTaskDependencies(
  store: TaskStorePort,
  id: string,
  depIds: readonly string[],
): Promise<Task> {
  return store.addDependencies(id, validateDependencyIds(depIds));
}

export async function removeTaskDependencies(
  store: TaskStorePort,
  id: string,
  depIds: readonly string[],
): Promise<Task> {
  return store.removeDependencies(id, validateDependencyIds(depIds));
}
