import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam, taskParam } from '../params.ts';
import { syncPlan } from '../../../app/tasks/sync-plan.ts';
import { translatePlan } from '../../../app/tasks/translate-plan.ts';
import { verifyTask } from '../../../app/tasks/verify-task.ts';
import { resolveVerificationConfig } from '../../../infra/adapters/tasks/verification-config.ts';
import type { ListOpts, TaskPort } from '../../../domain/ports/task.ts';
import type { TaskStatusType } from '../../../domain/types.ts';
import { writeExecutionMemory } from '../../../app/memory/execution/writer.ts';
import { buildTransitionHint, type TransitionHint } from '../../../app/workflow/playbook.ts';
import { taskBrief } from '../../../app/tasks/task-brief.ts';

async function maybeFinalTaskHint(
  taskPort: TaskPort, feature: string, tool: 'task_done' | 'task_accept',
): Promise<TransitionHint | undefined> {
  const allTasks = await taskPort.list(feature, { includeAll: true });
  const doneCount = allTasks.filter(t => t.status === 'done').length;
  return buildTransitionHint(tool, { taskDone: doneCount, taskTotal: allTasks.length });
}

export function registerTaskTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: sync | claim | done | accept | reject | block | unblock | spec_write | report_write
  server.registerTool(
    'maestro_task',
    {
      description:
        'Task state changes. Actions: sync (generate tasks from approved plan), claim (claim a task), ' +
        'done (mark complete with verification), accept (override failed verification -> done), ' +
        'reject (send back for revision), block (record blocker), unblock (resolve blocker), ' +
        'spec_write (write task spec), report_write (write completion report).',
      inputSchema: {
        action: z.enum(['sync', 'claim', 'done', 'accept', 'reject', 'block', 'unblock', 'spec_write', 'report_write'])
          .describe('Action to perform'),
        feature: featureParam(),
        task: taskParam().optional(),
        agent_id: z.string().optional().describe('Agent identifier (required for claim)'),
        summary: z.string().optional().describe('Summary of work completed (required for done; optional for accept)'),
        feedback: z.string().optional().describe('Revision feedback (required for reject)'),
        reason: z.string().optional().describe('Blocker reason (required for block)'),
        decision: z.string().optional().describe('Blocker resolution (required for unblock)'),
        content: z.string().optional().describe('Content (required for spec_write and report_write)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);

      switch (input.action) {
        case 'sync': {
          const result = services.taskBackend === 'br'
            ? await translatePlan(services, feature)
            : await syncPlan(services, feature);
          const hint = buildTransitionHint('tasks_sync', { created: result.created.length });
          return respond({ ...result, ...(hint && { transition: hint }) });
        }
        case 'claim': {
          if (!input.task) return respond({ error: 'task is required for action: claim' });
          if (!input.agent_id) return respond({ error: 'agent_id is required for action: claim' });
          const task = await services.taskPort.claim(feature, input.task, input.agent_id);
          return respond({ feature, task });
        }
        case 'done': {
          if (!input.task) return respond({ error: 'task is required for action: done' });
          if (!input.summary) return respond({ error: 'summary is required for action: done' });
          const vConfig = resolveVerificationConfig(services.settingsPort.get().verification);
          const result = await verifyTask({
            taskPort: services.taskPort,
            verificationPort: services.verificationPort,
            memoryAdapter: services.memoryAdapter,
            config: vConfig,
            projectRoot: services.directory,
            featureName: feature,
            taskFolder: input.task,
            summary: input.summary,
          });
          if (result.newStatus === 'done') {
            const hint = await maybeFinalTaskHint(services.taskPort, feature, 'task_done');
            return respond({ feature, task: result.task, verification: result.report, ...(hint && { transition: hint }) });
          }
          const revisionCount = result.task.revisionCount ?? 0;
          if (vConfig.autoReject && revisionCount >= vConfig.maxRevisions) {
            await writeExecutionMemory({
              memoryAdapter: services.memoryAdapter, featureName: feature,
              taskFolder: input.task, task: result.task, summary: input.summary,
              projectRoot: services.directory, verificationReport: result.report,
            });
            const acceptedTask = await services.taskPort.done(feature, input.task, input.summary);
            try {
              const { prependMetadataFrontmatter } = await import('../../../infra/utils/frontmatter.ts');
              const body = `Task ${input.task} auto-accepted after ${vConfig.maxRevisions} revision(s). Score: ${result.report.score.toFixed(2)}`;
              services.memoryAdapter.write(feature, `verification-auto-accept-${input.task}`,
                prependMetadataFrontmatter(body, { tags: ['verification', 'auto-accept'], category: 'debug', priority: 1 }));
            } catch { /* best-effort */ }
            const autoHint = await maybeFinalTaskHint(services.taskPort, feature, 'task_done');
            return respond({
              feature, task: acceptedTask, verification: result.report,
              warning: `Auto-accepted after ${vConfig.maxRevisions} revision(s) with score ${result.report.score.toFixed(2)}`,
              ...(autoHint && { transition: autoHint }),
            });
          }
          if (vConfig.autoReject) {
            const feedback = result.report.suggestions.join('; ') || 'Verification failed';
            const revisionTask = await services.taskPort.revision(feature, input.task, feedback, revisionCount + 1);
            return respond({
              feature, task: revisionTask, verification: result.report,
              status: 'revision', message: `Verification failed (score: ${result.report.score.toFixed(2)}). Task sent to revision.`,
            });
          }
          return respond({
            feature, task: result.task, verification: result.report,
            status: 'review', message: `Verification failed (score: ${result.report.score.toFixed(2)}). Use action: accept or action: reject.`,
          });
        }
        case 'accept': {
          if (!input.task) return respond({ error: 'task is required for action: accept' });
          const existing = await services.taskPort.get(feature, input.task);
          if (!existing || existing.status !== 'review') {
            throw new Error(`Task '${input.task}' is not in review state (current: ${existing?.status ?? 'not found'})`);
          }
          const summary = input.summary ?? existing.summary ?? '';
          let report = null;
          try { report = await services.taskPort.readVerification(feature, input.task); } catch { /* advisory */ }
          await writeExecutionMemory({
            memoryAdapter: services.memoryAdapter, featureName: feature,
            taskFolder: input.task, task: existing, summary,
            projectRoot: services.directory, verificationReport: report,
          });
          const task = await services.taskPort.done(feature, input.task, summary);
          const hint = await maybeFinalTaskHint(services.taskPort, feature, 'task_accept');
          return respond({ feature, task, message: 'Task accepted (verification override)', ...(hint && { transition: hint }) });
        }
        case 'reject': {
          if (!input.task) return respond({ error: 'task is required for action: reject' });
          if (!input.feedback) return respond({ error: 'feedback is required for action: reject' });
          const existing = await services.taskPort.get(feature, input.task);
          if (!existing || existing.status !== 'review') {
            throw new Error(`Task '${input.task}' is not in review state (current: ${existing?.status ?? 'not found'})`);
          }
          const revisionCount = (existing.revisionCount ?? 0) + 1;
          const task = await services.taskPort.revision(feature, input.task, input.feedback, revisionCount);
          return respond({ feature, task, message: `Task sent for revision (attempt ${revisionCount})` });
        }
        case 'block': {
          if (!input.task) return respond({ error: 'task is required for action: block' });
          if (!input.reason) return respond({ error: 'reason is required for action: block' });
          const task = await services.taskPort.block(feature, input.task, input.reason);
          return respond({ feature, task });
        }
        case 'unblock': {
          if (!input.task) return respond({ error: 'task is required for action: unblock' });
          if (!input.decision) return respond({ error: 'decision is required for action: unblock' });
          const task = await services.taskPort.unblock(feature, input.task, input.decision);
          return respond({ feature, task });
        }
        case 'spec_write': {
          if (!input.task) return respond({ error: 'task is required for action: spec_write' });
          if (!input.content) return respond({ error: 'content is required for action: spec_write' });
          await services.taskPort.writeSpec(feature, input.task, input.content);
          return respond({ feature, task: input.task, written: true });
        }
        case 'report_write': {
          if (!input.task) return respond({ error: 'task is required for action: report_write' });
          if (!input.content) return respond({ error: 'content is required for action: report_write' });
          await services.taskPort.writeReport(feature, input.task, input.content);
          return respond({ feature, task: input.task, written: true });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: list | info | spec | report | next | brief
  server.registerTool(
    'maestro_task_read',
    {
      description:
        'Task read operations. What: list (all tasks with status), info (specific task details), ' +
        'spec (compiled task spec), report (completion report), next (runnable tasks with recommended spec), ' +
        'brief (full agent context: spec + memories + doctrine + graph + worker rules).',
      inputSchema: {
        what: z.enum(['list', 'info', 'spec', 'report', 'next', 'brief']).describe('What to read'),
        feature: featureParam(),
        task: taskParam().optional(),
        status: z.enum(['pending', 'claimed', 'done', 'blocked', 'review', 'revision']).optional().describe('Filter by status (list only)'),
        includeAll: z.boolean().optional().describe('Include all tasks regardless of status (list only)'),
        brief: z.boolean().optional().default(false).describe('Return compact task info (list only)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);

      switch (input.what) {
        case 'list': {
          const featureInfo = services.featureAdapter.get(feature);
          if (!featureInfo) {
            return respond({ feature, tasks: [], warning: `Feature '${feature}' not found` });
          }
          const opts: ListOpts = {};
          if (input.status !== undefined) opts.status = input.status as TaskStatusType;
          if (input.includeAll !== undefined) opts.includeAll = input.includeAll;
          const tasks = await services.taskPort.list(feature, opts);
          if (input.brief) {
            const compact = tasks.map(({ id, name, status, origin, dependsOn }) => ({
              id, name, status, origin, dependsOn,
            }));
            return respond({ feature, tasks: compact });
          }
          return respond({ feature, tasks });
        }
        case 'info': {
          if (!input.task) return respond({ error: 'task is required for what: info' });
          const task = await services.taskPort.get(feature, input.task);
          if (!task) {
            return respond({ error: `Task '${input.task}' not found in feature '${feature}'` });
          }
          return respond({ feature, task });
        }
        case 'spec': {
          if (!input.task) return respond({ error: 'task is required for what: spec' });
          const spec = await services.taskPort.readSpec(feature, input.task);
          return respond({ feature, task: input.task, spec: spec ?? null });
        }
        case 'report': {
          if (!input.task) return respond({ error: 'task is required for what: report' });
          const report = await services.taskPort.readReport(feature, input.task);
          return respond({ feature, task: input.task, report: report ?? null });
        }
        case 'next': {
          const runnable = await services.taskPort.getRunnable(feature);
          const tasks = runnable.map(({ id, name, status, dependsOn }) => ({ id, name, status, dependsOn }));
          const recommendedSpec = runnable.length > 0
            ? await services.taskPort.readSpec(feature, runnable[0].id)
            : undefined;
          return respond({ feature, tasks, ...(recommendedSpec !== undefined && { recommendedSpec }) });
        }
        case 'brief': {
          if (!input.task) return respond({ error: 'task is required for what: brief' });
          const result = await taskBrief({
            taskPort: services.taskPort,
            featureAdapter: services.featureAdapter,
            memoryAdapter: services.memoryAdapter,
            settingsPort: services.settingsPort,
            directory: services.directory,
            graphPort: services.graphPort,
            doctrinePort: services.doctrinePort,
          }, feature, input.task);
          const guidance = services.agentToolsRegistry.assembleProtocol('code-intelligence') ?? undefined;
          return respond({ ...result, agentToolsGuidance: guidance });
        }
        default:
          return respond({ error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
