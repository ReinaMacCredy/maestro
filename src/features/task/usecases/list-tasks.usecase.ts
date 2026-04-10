import type { Task, ListTasksFilters } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

/**
 * List tasks applying filter composition and optional limit.
 * Results are sorted by createdAt ASC so output is deterministic.
 */
export async function listTasks(
  store: TaskStorePort,
  filters: ListTasksFilters = {},
): Promise<readonly Task[]> {
  const all = await store.all();

  const filtered = all.filter((task) => {
    if (filters.status !== undefined && task.status !== filters.status) return false;
    if (filters.priority !== undefined && task.priority !== filters.priority) return false;
    if (filters.type !== undefined && task.type !== filters.type) return false;
    if (filters.parentId !== undefined && task.parentId !== filters.parentId) return false;
    if (filters.assignee !== undefined && task.assignee !== filters.assignee) return false;
    if (filters.label !== undefined && !task.labels.includes(filters.label)) return false;
    return true;
  });

  filtered.sort((a, b) => a.createdAt.localeCompare(b.createdAt));

  if (filters.limit !== undefined && filters.limit > 0) {
    return filtered.slice(0, filters.limit);
  }
  return filtered;
}
