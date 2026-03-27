/**
 * check-status use case.
 * Composite query for feature, plan, tasks, and context.
 */

import type { TaskPort } from '../../domain/ports/task.ts';
import type { FeaturePort } from '../../domain/ports/feature.ts';
import type { PlanPort } from '../../domain/ports/plan.ts';
import type { MemoryPort } from '../../domain/ports/memory.ts';
import type { GraphPort } from '../../domain/ports/graph.ts';
import type { SearchPort } from '../../domain/ports/search.ts';
import type { HandoffPort } from '../../domain/ports/handoff.ts';
import { countTaskStatuses, getNextAction } from './stages.ts';
import { computeRunnableAndBlocked } from '../tasks/graph/dependency.ts';
import { MaestroError } from '../../domain/errors.ts';
import type { SettingsPort } from '../../domain/ports/settings.ts';
import { type TaskInfo, type FeatureStatusType, type PlanComment } from '../../domain/types.ts';
import { resolveDcpConfig } from '../dcp/config.ts';

export interface StatusServices {
  taskPort: TaskPort;
  featureAdapter: FeaturePort;
  planAdapter: PlanPort;
  memoryAdapter: MemoryPort;
  settingsPort: SettingsPort;
  directory: string;
  graphPort?: GraphPort;
  handoffPort?: HandoffPort;
  searchPort?: SearchPort;
}

export interface StatusResult {
  feature: {
    name: string;
    status: FeatureStatusType;
  };
  plan: {
    exists: boolean;
    approved: boolean;
    commentCount: number;
    comments: PlanComment[];
  };
  tasks: {
    total: number;
    pending: number;
    inProgress: number;
    done: number;
    review: number;
    revision: number;
    items: TaskInfo[];
  };
  runnable: string[];
  blocked: string[];
  expiredClaims: string[];
  context: {
    count: number;
    totalBytes: number;
  };
  integrations: {
    bv: boolean;
    agentMail: boolean;
    cass: boolean;
  };
  dcp?: {
    enabled: boolean;
    memoryBudgetTokens: number;
  };
  nextAction: string;
}

export async function checkStatus(
  services: StatusServices,
  featureName: string,
): Promise<StatusResult> {
  const { taskPort, featureAdapter, planAdapter, memoryAdapter, settingsPort } = services;
  const feature = featureAdapter.get(featureName);
  if (!feature) {
    throw new MaestroError(`Feature '${featureName}' not found`);
  }

  const plan = planAdapter.read(featureName);
  const tasks = await taskPort.list(featureName, { includeAll: true });
  const memoryStats = memoryAdapter.stats(featureName);
  const comments = plan?.comments || [];

  const settings = settingsPort.get();

  // Detect expired claims
  const claimExpiresMinutes = settings.tasks.claimExpiresMinutes;
  const expiryMs = claimExpiresMinutes * 60 * 1000;
  const now = Date.now();
  const expiredClaims = tasks
    .filter(t => t.status === 'claimed' && t.claimedAt && now - new Date(t.claimedAt).getTime() > expiryMs)
    .map(t => t.id);

  // Derive blocked from task list
  const blocked = tasks
    .filter((t) => t.status === 'blocked')
    .map((t) => t.id);

  // Compute runnable from already-fetched task list (avoids second list() call)
  const { runnable: runnableIds } = computeRunnableAndBlocked(tasks);

  const counts = countTaskStatuses(tasks);

  const planStatus = plan ? (plan.status === 'approved' ? 'approved' : 'draft') : null;
  const nextAction = getNextAction(
    planStatus,
    tasks.map((task) => ({ status: task.status, folder: task.id })),
    runnableIds,
    feature.status,
  );

  const dcpCfg = resolveDcpConfig(settings.dcp);

  return {
    feature: {
      name: feature.name,
      status: feature.status,
    },
    plan: {
      exists: !!plan,
      approved: plan?.status === 'approved',
      commentCount: comments.length,
      comments,
    },
    tasks: {
      total: tasks.length,
      ...counts,
      items: tasks,
    },
    runnable: runnableIds,
    blocked,
    expiredClaims,
    context: {
      count: memoryStats.count,
      totalBytes: memoryStats.totalBytes,
    },
    integrations: {
      bv: !!services.graphPort,
      agentMail: !!services.handoffPort,
      cass: !!services.searchPort,
    },
    dcp: {
      enabled: dcpCfg.enabled,
      memoryBudgetTokens: dcpCfg.memoryBudgetTokens,
    },
    nextAction,
  };
}
