/**
 * sync-plan use case.
 * Parse plan.md via parseTasksFromPlan(), validate, diff against existing
 * tasks in TaskPort, create/remove as needed.
 */

import type { TaskPort } from '../../domain/ports/task.ts';
import { isActiveTask } from './transitions.ts';
import type { PlanPort } from '../../domain/ports/plan.ts';
import { parseTasksFromPlan, validateDependencyGraph, resolveDependencies } from '../plans/parser.ts';
import { buildSpecContent } from '../tasks/spec-builder.ts';
import { MaestroError } from '../../domain/errors.ts';
import type { TasksSyncResult } from '../../domain/types.ts';

export interface SyncPlanServices {
  taskPort: TaskPort;
  planAdapter: PlanPort;
}

export interface SyncPlanOpts {
  dryRun?: boolean;
}

export async function syncPlan(
  services: SyncPlanServices,
  featureName: string,
  opts?: SyncPlanOpts,
): Promise<TasksSyncResult> {
  const { taskPort, planAdapter } = services;
  const dryRun = opts?.dryRun ?? false;
  const plan = planAdapter.read(featureName);
  if (!plan) throw new MaestroError(`No plan found for feature '${featureName}'`);
  if (plan.status !== 'approved') {
    throw new MaestroError(
      'Plan must be approved before syncing tasks',
      ['Run: maestro plan-approve --feature ' + featureName]
    );
  }

  const parsedTasks = parseTasksFromPlan(plan.content);
  validateDependencyGraph(parsedTasks, featureName);

  const warnings: string[] = [];
  if (parsedTasks.length === 0) {
    warnings.push(
      'Plan produced 0 tasks. Expected numbered headings like "### 1. Setup database" or "#### 1. Setup database".'
    );
  }

  const existingTasks = await taskPort.list(featureName, { includeAll: true });
  const existingByFolder = new Map(existingTasks.map(t => [t.folder, t]));
  const existingById = new Map(existingTasks.map(t => [t.id, t]));
  const parsedIdSet = new Set(parsedTasks.map(p => p.id));
  const parsedFolderSet = new Set(parsedTasks.map(p => p.folder));

  const result: TasksSyncResult = {
    created: [],
    removed: [],
    kept: [],
    manual: [],
  };

  for (const existing of existingTasks) {
    if (existing.origin === 'manual') {
      result.manual.push(existing.folder);
      continue;
    }

    if (isActiveTask(existing.status)) {
      result.kept.push(existing.folder);
      continue;
    }

    const stillInPlan = parsedFolderSet.has(existing.folder) || parsedIdSet.has(existing.id);
    if (!stillInPlan) {
      if (!dryRun) await taskPort.remove(featureName, existing.folder);
      result.removed.push(existing.folder);
    } else {
      result.kept.push(existing.folder);
    }
  }

  for (const parsedTask of parsedTasks) {
    if (existingByFolder.has(parsedTask.folder) || existingById.has(parsedTask.id)) continue;

    const dependsOn = resolveDependencies(parsedTask, parsedTasks);

    const specContent = buildSpecContent({
      featureName,
      task: parsedTask,
      dependsOn,
      allTasks: parsedTasks,
      planContent: plan.content,
    });

    if (!dryRun) {
      const created = await taskPort.create(featureName, parsedTask.name, {
        description: specContent,
        deps: dependsOn,
      });

      // Update the parsed task's folder to match the actual folder assigned by
      // the adapter (e.g. br prefixes the issue ID). This ensures subsequent
      // tasks resolving dependencies find the correct name in the mapping.
      parsedTask.folder = created.folder;

      result.created.push(created.folder);
    } else {
      result.created.push(parsedTask.folder);
    }
  }

  if (warnings.length > 0) result.warnings = warnings;

  return result;
}
