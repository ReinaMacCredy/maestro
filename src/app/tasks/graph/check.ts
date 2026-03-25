/**
 * Task dependency checking for maestroCLI.
 */

import { buildEffectiveDependencies } from './dependency.ts';
import type { TaskPort } from '../../../tasks/port.ts';
import { isDependencySatisfied } from '../transitions.ts';
import type { TaskInfo, TaskStatusType } from '../../../core/types.ts';

/**
 * Check if a task's dependencies are satisfied.
 */
export async function checkDependencies(
  taskPort: TaskPort,
  feature: string,
  taskFolder: string,
  existingTasks?: TaskInfo[],
): Promise<{ allowed: true; error?: undefined } | { allowed: false; error: string }> {
  const tasks = existingTasks ?? await taskPort.list(feature, { includeAll: true });

  const tasksWithDeps = tasks.map(task => ({
    id: task.id ?? task.folder,
    folder: task.folder,
    status: task.status,
    dependsOn: task.dependsOn,
  }));

  const effectiveDeps = buildEffectiveDependencies(tasksWithDeps);

  // Look up deps by id first, fall back to folder for backward compatibility
  const targetTask = tasks.find(t => t.folder === taskFolder || t.id === taskFolder);
  const targetId = targetTask ? (targetTask.id ?? targetTask.folder) : taskFolder;
  const deps = effectiveDeps.get(targetId) ?? effectiveDeps.get(taskFolder) ?? [];

  if (deps.length === 0) {
    return { allowed: true };
  }

  // Support lookup by id or folder for backward compatibility
  const statusById = new Map<string, TaskStatusType>();
  for (const t of tasks) {
    statusById.set(t.folder, t.status);
    if (t.id) statusById.set(t.id, t.status);
  }
  const unmetDeps: Array<{ folder: string; status: string }> = [];

  for (const depId of deps) {
    const depStatus = statusById.get(depId);
    if (!depStatus || !isDependencySatisfied(depStatus)) {
      unmetDeps.push({ folder: depId, status: depStatus ?? 'unknown' });
    }
  }

  if (unmetDeps.length > 0) {
    const depList = unmetDeps
      .map(d => `"${d.folder}" (${d.status})`)
      .join(', ');

    return {
      allowed: false,
      error: `Dependency constraint: Task "${taskFolder}" cannot start - dependencies not done: ${depList}. ` +
        `Only tasks with status 'done' satisfy dependencies.`,
    };
  }

  return { allowed: true };
}
