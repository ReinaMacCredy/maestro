/**
 * Task dependency graph computation for maestroCLI.
 * Forked from hive-core/src/services/taskDependencyGraph.ts -- direct copy.
 */

import type { TaskStatusType } from '../../../core/types.ts';
import { isDependencySatisfied } from '../transitions.ts';

export interface TaskWithDeps {
  /** Primary identifier. */
  id: string;
  /** @deprecated Internal storage path segment. Use `id` for public identity. */
  folder: string;
  status: TaskStatusType;
  dependsOn?: string[];
}

export interface RunnableBlockedResult {
  runnable: string[];
  blocked: Record<string, string[]>;
}

export function computeRunnableAndBlocked(tasks: TaskWithDeps[]): RunnableBlockedResult {
  // Dual-key status lookup: deps may reference id or folder (backward compat)
  const statusLookup = new Map<string, TaskStatusType>();
  for (const task of tasks) {
    statusLookup.set(task.id, task.status);
    statusLookup.set(task.folder, task.status);
  }

  const runnable: string[] = [];
  const blocked: Record<string, string[]> = {};

  const effectiveDeps = buildEffectiveDependencies(tasks);

  for (const task of tasks) {
    // Both pending and revision tasks are candidates for runnable
    if (task.status !== 'pending' && task.status !== 'revision') {
      continue;
    }

    const key = task.id;
    const deps = effectiveDeps.get(key) ?? effectiveDeps.get(task.folder) ?? [];

    const unmetDeps = deps.filter(dep => {
      const depStatus = statusLookup.get(dep);
      return !depStatus || !isDependencySatisfied(depStatus);
    });

    if (unmetDeps.length === 0) {
      runnable.push(key);
    } else {
      blocked[key] = unmetDeps;
    }
  }

  return { runnable, blocked };
}

/**
 * Build effective dependencies from explicit dependsOn declarations.
 * No implicit ordering from numeric folder prefixes.
 */
export function buildEffectiveDependencies(tasks: TaskWithDeps[]): Map<string, string[]> {
  return new Map(tasks.map(t => [t.id, t.dependsOn ?? []]));
}
