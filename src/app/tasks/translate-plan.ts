/**
 * translate-plan use case (flywheel-style plan-to-beads conversion).
 *
 * Like sync-plan but creates rich, self-contained beads via br instead of
 * thin task stubs. Each bead carries description, design notes, acceptance
 * criteria, and full context so agents never need to look back at the plan.
 *
 * Used when taskBackend === 'br'. Falls back to sync-plan for fs backend.
 */

import type { TaskPort } from '../../domain/ports/task.ts';
import { isActiveTask } from './transitions.ts';
import type { PlanPort } from '../../domain/ports/plan.ts';
import { parseTasksFromPlan, validateDependencyGraph, resolveDependencies } from '../plans/parser.ts';
import { buildBeadOpts } from '../tasks/bead-builder.ts';
import { MaestroError } from '../../domain/errors.ts';
import type { TasksSyncResult } from '../../domain/types.ts';

export interface TranslatePlanServices {
  taskPort: TaskPort;
  planAdapter: PlanPort;
}

export interface TranslatePlanOpts {
  dryRun?: boolean;
}

export async function translatePlan(
  services: TranslatePlanServices,
  featureName: string,
  opts?: TranslatePlanOpts,
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

  // Memory files deliberately omitted from bead descriptions.
  // The pre-agent hook handles memory injection via DCP at agent-spawn time,
  // so baking memories into beads would cause double injection.

  const existingTasks = await taskPort.list(featureName, { includeAll: true });
  const existingByFolder = new Map(existingTasks.map(t => [t.folder, t]));
  const existingById = new Map(existingTasks.map(t => [t.id, t]));
  const parsedIdSet = new Set(parsedTasks.map(p => p.id));
  const parsedFolderSet = new Set(parsedTasks.map(p => p.folder));

  // Completed tasks deliberately omitted from bead descriptions.
  // The pre-agent hook handles completed task injection via DCP observation
  // masking, so baking them into beads would cause double injection.

  const result: TasksSyncResult = {
    created: [],
    removed: [],
    kept: [],
    manual: [],
  };

  // Handle existing tasks (same logic as sync-plan)
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

  // Create rich beads from plan sections.
  // Track plan-ID -> br-folder so deps resolve to created br folders, not raw plan IDs.
  const planIdToFolder = new Map<string, string>();
  for (const parsedTask of parsedTasks) {
    if (existingByFolder.has(parsedTask.folder) || existingById.has(parsedTask.id)) continue;

    const rawDeps = resolveDependencies(parsedTask, parsedTasks);
    const resolvedDeps = rawDeps.map(dep => planIdToFolder.get(dep) ?? dep);

    const beadOpts = buildBeadOpts({
      featureName,
      task: parsedTask,
      planContent: plan.content,
      allTasks: parsedTasks,
      dependsOn: resolvedDeps,
    });

    if (!dryRun) {
      const created = await taskPort.create(featureName, parsedTask.name, beadOpts);

      // Map plan ID to actual br folder so later tasks can resolve deps
      planIdToFolder.set(parsedTask.id, created.folder);
      // Update folder to match actual assignment (br prefixes issue ID)
      parsedTask.folder = created.folder;

      result.created.push(created.folder);
    } else {
      result.created.push(parsedTask.folder);
    }
  }

  if (warnings.length > 0) result.warnings = warnings;

  return result;
}
