/**
 * Task brief use-case -- assembles full worker context as structured data.
 * Universal replacement for hook-based injection: any MCP-capable agent
 * can call maestro_task_brief and get the same rich context.
 */

import type { TaskPort, RichTaskFields } from '../../domain/ports/task.ts';
import type { GraphPort } from '../../domain/ports/graph.ts';
import type { DoctrinePort } from '../../domain/ports/doctrine.ts';
import type { SettingsPort } from '../../domain/ports/settings.ts';
import type { MemoryFileWithMeta } from '../../domain/types.ts';
import type { TaskWithDeps } from '../tasks/graph/dependency.ts';
import { selectMemories, type SelectedContext } from '../dcp/selector.ts';
import { deriveFolderTags } from '../memory/execution/writer.ts';
import { extractKeywords } from '../dcp/relevance.ts';
import { appendDoctrineTrace } from '../doctrine/trace.ts';
import { WORKER_RULES } from '../tasks/worker-rules.ts';
import { resolveDcpConfig } from '../dcp/config.ts';
import { resolveDoctrineConfig } from '../doctrine/config.ts';
import { MaestroError } from '../../domain/errors.ts';
import { estimateTokens } from '../../infra/utils/tokens.ts';
import { fitWithinBudget } from '../dcp/budget.ts';

export interface TaskBriefParams {
  taskPort: TaskPort;
  featureAdapter: { get(name: string): { name: string; createdAt?: string } | null };
  memoryAdapter: { listWithMeta(feature: string): MemoryFileWithMeta[] };
  settingsPort: SettingsPort;
  directory: string;
  graphPort?: GraphPort;
  doctrinePort?: DoctrinePort;
}

export interface TaskBriefResult {
  feature: string;
  task: string;
  spec: string;
  memories: Array<{ name: string; content: string; score: number; tags: string[]; category: string }>;
  completedTasks: Array<{ id: string; name: string; summary: string }>;
  doctrine: Array<{ name: string; rule: string; rationale: string }>;
  graphContext?: { onCriticalPath: boolean; isBottleneck: boolean };
  revisionContext?: {
    attempt: number;
    feedback?: string;
    failedChecks: Array<{ name: string; detail: string }>;
    suggestions: string[];
  };
  richFields?: { design?: string; acceptanceCriteria?: string };
  workerRules: string;
  dcp: {
    totalTokens: number;
    totalBytes: number;
    memoriesIncluded: number;
    memoriesDropped: number;
    scores: Array<{ name: string; score: number; included: boolean }>;
  };
  agentToolsGuidance?: string;
  hint?: string;
}

export async function taskBrief(
  params: TaskBriefParams,
  feature: string,
  taskFolder: string,
): Promise<TaskBriefResult> {
  const { taskPort, featureAdapter, memoryAdapter, settingsPort, directory } = params;

  // 1. Get task -- fail if not found
  const task = await taskPort.get(feature, taskFolder);
  if (!task) {
    throw new MaestroError(`Task '${taskFolder}' not found in feature '${feature}'`);
  }
  const hint = task.status !== 'claimed'
    ? 'Task is not claimed. Consider calling task_claim first.'
    : undefined;

  // 2. Read compiled spec -- fail if absent
  const spec = await taskPort.readSpec(feature, taskFolder);
  if (!spec) {
    throw new MaestroError(`No spec found for task '${taskFolder}' in feature '${feature}'`);
  }

  // 3. Feature created-at for DCP recency scoring
  const featureInfo = featureAdapter.get(feature);
  const featureCreatedAt = featureInfo?.createdAt;

  // 4. Parallel independent reads
  const [richResult, insightsResult, allTasksResult] = await Promise.allSettled([
    taskPort.getRichFields?.(feature, taskFolder) ?? Promise.resolve(null),
    params.graphPort?.getInsights() ?? Promise.resolve(null),
    taskPort.list(feature, { includeAll: true }),
  ]);

  // 5. Revision context
  let revisionContext: TaskBriefResult['revisionContext'];
  if (task.revisionCount && task.revisionCount > 0) {
    const failedChecks: Array<{ name: string; detail: string }> = [];
    const suggestions: string[] = [];
    try {
      const report = await taskPort.readVerification(feature, taskFolder);
      if (report) {
        for (const c of report.criteria) {
          if (!c.passed) failedChecks.push({ name: c.name, detail: c.detail });
        }
        suggestions.push(...report.suggestions);
      }
    } catch { /* best-effort */ }
    revisionContext = {
      attempt: task.revisionCount + 1,
      feedback: task.revisionFeedback ?? undefined,
      failedChecks,
      suggestions,
    };
  }

  // 6. Graph context
  let graphContext: TaskBriefResult['graphContext'];
  if (insightsResult.status === 'fulfilled' && insightsResult.value) {
    const insights = insightsResult.value;
    const matchId = (item: { id: string; title: string }) =>
      item.id === taskFolder || item.title === task.name;
    graphContext = {
      onCriticalPath: insights.criticalPath.some(matchId),
      isBottleneck: insights.bottlenecks.some(matchId),
    };
  }

  // 7. Rich fields
  let richFields: TaskBriefResult['richFields'];
  if (richResult.status === 'fulfilled' && richResult.value) {
    const r = richResult.value as RichTaskFields;
    if (r.design || r.acceptanceCriteria) {
      richFields = { design: r.design, acceptanceCriteria: r.acceptanceCriteria };
    }
  }

  // 8. DCP-scored memories
  const settings = settingsPort.get();
  const dcpConfig = resolveDcpConfig(settings.dcp);
  const allTasks: TaskWithDeps[] = allTasksResult.status === 'fulfilled'
    ? allTasksResult.value.map(t => ({ id: t.id, folder: t.folder, status: t.status, dependsOn: t.dependsOn }))
    : [];

  const rawMemories = memoryAdapter.listWithMeta(feature);
  const planSection = task.planTitle ?? null;
  const selected: SelectedContext = selectMemories(
    rawMemories, task, planSection,
    dcpConfig.memoryBudgetTokens, dcpConfig.relevanceThreshold,
    featureCreatedAt, allTasks,
  );

  const scoreMap = new Map(selected.scores.map(s => [s.name, s.score]));
  const memories = selected.memories.map(m => ({
    name: m.name,
    content: m.bodyContent ?? m.content,
    score: Math.round((scoreMap.get(m.name) ?? 0) * 1000) / 1000,
    tags: m.metadata?.tags ?? [],
    category: m.metadata?.category ?? 'unknown',
  }));

  // 9. Completed tasks (newest-first, budget-capped)
  const completedTasks: Array<{ id: string; name: string; summary: string }> = [];
  if (allTasksResult.status === 'fulfilled') {
    const doneTasks = allTasksResult.value
      .filter(t => t.status === 'done' && t.summary)
      .reverse(); // newest-first (list returns creation order)
    const budgetTokens = dcpConfig.completedTaskBudgetTokens ?? 512;
    const entries = doneTasks.map(t => ({ id: t.id, name: t.name ?? t.id, summary: t.summary! }));
    completedTasks.push(...fitWithinBudget(entries, e => estimateTokens(JSON.stringify(e)), budgetTokens));
  }

  // 10. Doctrine (try/catch, trace only if claimed)
  let doctrine: Array<{ name: string; rule: string; rationale: string }> = [];
  try {
    if (params.doctrinePort) {
      const doctrineConfig = resolveDoctrineConfig(settings.doctrine);
      if (doctrineConfig.enabled) {
        const derivedTags = deriveFolderTags(taskFolder);
        const specKeywords = extractKeywords(spec);
        const items = params.doctrinePort.findRelevant(derivedTags, specKeywords);

        // Budget-cap doctrine items
        const doctrineBudgetTokens = doctrineConfig.doctrineBudgetTokens ?? 256;
        let usedTokens = 0;
        for (const item of items) {
          const entry = { name: item.name, rule: item.rule, rationale: item.rationale };
          const entryTokens = estimateTokens(JSON.stringify(entry));
          if (usedTokens + entryTokens > doctrineBudgetTokens) break;
          usedTokens += entryTokens;
          doctrine.push(entry);
        }

        // Record trace only for claimed tasks
        if (task.status === 'claimed' && doctrine.length > 0) {
          appendDoctrineTrace(
            directory, feature, taskFolder,
            task.revisionCount ?? 0,
            doctrine.map(d => d.name),
          );
        }
      }
    }
  } catch { /* best-effort -- doctrine is advisory */ }

  // 11. Assemble result
  return {
    feature,
    task: taskFolder,
    spec,
    memories,
    completedTasks,
    doctrine,
    graphContext,
    revisionContext,
    richFields,
    workerRules: WORKER_RULES,
    dcp: {
      totalTokens: selected.totalTokens,
      totalBytes: selected.totalBytes,
      memoriesIncluded: selected.includedCount,
      memoriesDropped: selected.droppedCount,
      scores: selected.scores.map(s => ({
        name: s.name,
        score: Math.round(s.score * 1000) / 1000,
        included: s.included,
      })),
    },
    hint,
  };
}
