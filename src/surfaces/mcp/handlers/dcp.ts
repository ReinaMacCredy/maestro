/**
 * MCP tool for DCP (Dynamic Context Pruning).
 * Preview, stats, and config for memory selection.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { MaestroServices } from '../../../services.ts';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam, taskParam } from '../params.ts';
import { pruneContext } from '../../../app/dcp/prune-context.ts';
import { resolveDcpConfig } from '../../../app/dcp/config.ts';
import { WORKER_RULES } from '../../../app/tasks/worker-rules.ts';
import { collectMetrics, formatMetricsSummary } from '../../../app/dcp/metrics.ts';
import { COMPONENT_REGISTRY } from '../../../app/dcp/components.ts';
import { DEFAULT_SETTINGS } from '../../../domain/ports/settings.ts';
import type { TaskInfo, MemoryFileWithMeta } from '../../../domain/types.ts';

interface DcpContext {
  feature: string;
  task: TaskInfo;
  spec: string;
  memories: MemoryFileWithMeta[];
  resolvedDcp: ReturnType<typeof resolveDcpConfig>;
  featureCreatedAt: string | undefined;
  allTasks: Awaited<ReturnType<MaestroServices['taskPort']['list']>>;
}

async function prepareDcpContext(
  services: MaestroServices, featureInput: string | undefined, taskFolder: string,
): Promise<DcpContext | null> {
  const feature = requireFeature(services, featureInput);
  const task = await services.taskPort.get(feature, taskFolder);
  if (!task) return null;
  const spec = await services.taskPort.readSpec(feature, taskFolder) ?? '';
  const memories = services.memoryAdapter.listWithMeta(feature);
  const resolvedDcp = resolveDcpConfig(services.settingsPort.get().dcp);
  const featureCreatedAt = services.featureAdapter.get(feature)?.createdAt;
  const allTasks = await services.taskPort.list(feature, { includeAll: true });
  return { feature, task, spec, memories, resolvedDcp, featureCreatedAt, allTasks };
}

export function registerDcpTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_dcp',
    {
      description:
        'Inspect how Dynamic Context Pruning selects and budgets memory for tasks. ' +
        'Use to debug why certain memories are included/excluded, check token budgets, or review DCP settings. ' +
        'What: preview (requires: task -- memory scores), stats (requires: task -- budget breakdown), config (current settings). ' +
        'Example: maestro_dcp({ what: "preview", task: "implement-auth" })',
      inputSchema: {
        what: z.enum(['preview', 'stats', 'config']).optional().describe('Query to perform'),
        feature: featureParam(),
        task: taskParam().optional(),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const what = input.what;
      if (!what) return errorResponse({ terminal: false, reason: 'validation', error: 'what is required' });

      switch (what) {
        case 'preview': {
          if (!input.task) return errorResponse({ terminal: false, reason: 'validation', error: 'task is required for action: preview', suggestions: ['Provide the task parameter.'] });
          const ctx = await prepareDcpContext(services, input.feature, input.task);
          if (!ctx) return errorResponse({ terminal: false, reason: 'not_found', error: `Task '${input.task}' not found in feature '${requireFeature(services, input.feature)}'`, suggestions: ['Use maestro_task_read(what: list) to see available tasks.'] });
          const { feature, task, memories, resolvedDcp, featureCreatedAt, allTasks } = ctx;
          const taskDeps = allTasks.map(t => ({ id: t.id, folder: t.folder, status: t.status, dependsOn: t.dependsOn }));
          const { metrics } = pruneContext({
            featureName: feature,
            taskFolder: input.task,
            task,
            spec: ctx.spec || '(no spec)',
            memories,
            richContext: '',
            graphContext: '',
            workerRules: WORKER_RULES,
            dcpConfig: resolvedDcp,
            featureCreatedAt,
            allTasks: taskDeps,
          });
          return respond({
            feature,
            task: input.task,
            dcp: { enabled: resolvedDcp.enabled, memoryBudgetTokens: resolvedDcp.memoryBudgetTokens },
            memories: { total: metrics.memoriesTotal, included: metrics.memoriesIncluded, dropped: metrics.memoriesDropped },
            scores: metrics.scores.map(s => ({
              name: s.name,
              score: Math.round(s.score * 1000) / 1000,
              included: s.included,
            })),
            sections: metrics.sections,
          });
        }
        case 'stats': {
          if (!input.task) return errorResponse({ terminal: false, reason: 'validation', error: 'task is required for action: stats', suggestions: ['Provide the task parameter.'] });
          const ctx = await prepareDcpContext(services, input.feature, input.task);
          if (!ctx) return errorResponse({ terminal: false, reason: 'not_found', error: `Task '${input.task}' not found`, suggestions: ['Use maestro_task_read(what: list) to see available tasks.'] });
          const { feature, task, spec, memories, resolvedDcp, allTasks } = ctx;
          const taskDeps = allTasks.map(t => ({ folder: t.folder, id: t.id, status: t.status, dependsOn: t.dependsOn }));
          const result = pruneContext({
            featureName: feature,
            taskFolder: input.task,
            task,
            spec,
            memories,
            richContext: '',
            graphContext: '',
            workerRules: WORKER_RULES,
            dcpConfig: resolvedDcp,
            allTasks: taskDeps,
          });
          const metrics = collectMetrics(result, memories);
          return respond({ feature, task: input.task, metrics, summary: formatMetricsSummary(metrics) });
        }
        case 'config': {
          const dcpSettings = services.settingsPort.get().dcp;
          const resolved = resolveDcpConfig(dcpSettings);
          return respond({
            settings: resolved,
            components: COMPONENT_REGISTRY.map(c => ({
              name: c.name,
              priority: c.priority,
              protected: c.protected,
            })),
            defaults: DEFAULT_SETTINGS.dcp,
          });
        }
        default:
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${what}` });
      }
    }),
  );
}
