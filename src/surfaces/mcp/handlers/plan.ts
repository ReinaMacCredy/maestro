import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireFeature } from '../../../core/resolve.ts';
import { featureParam } from '../params.ts';
import { writePlan } from '../../../plans/write-plan.ts';
import { approvePlan } from '../../../plans/approve-plan.ts';
import { MaestroError } from '../../../core/errors.ts';
import { buildTransitionHint } from '../../../workflow/playbook.ts';
import { extractPlanOutline } from '../../../plans/parser.ts';

export function registerPlanTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: write | approve | revoke | comment | comments_clear
  server.registerTool(
    'maestro_plan',
    {
      description:
        'Plan mutations. Actions: write (create/update plan), approve (approve for execution), ' +
        'revoke (un-approve plan), comment (add review comment), comments_clear (remove all comments). ' +
        'Plan must include a ## Discovery section (min 100 chars), ## Non-Goals, and ## Ghost Diffs sections.',
      inputSchema: {
        action: z.enum(['write', 'approve', 'revoke', 'comment', 'comments_clear'])
          .describe('Action to perform'),
        feature: featureParam(),
        content: z.string().optional().describe('Full plan content in markdown (write only; not required when scaffold is true)'),
        scaffold: z.boolean().optional().default(false).describe('Write a plan template scaffold instead of real content (write only)'),
        body: z.string().optional().describe('Comment text (comment only)'),
        line: z.number().optional().describe('Line number this comment refers to (comment only)'),
        author: z.string().optional().describe('Comment author name (comment only)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'write': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          if (!input.scaffold && !input.content) {
            throw new MaestroError('content is required when scaffold is false', [
              'Provide content or set scaffold: true',
            ]);
          }
          const result = await writePlan(
            { ...services, memoryAdapter: services.memoryAdapter },
            feature, input.content ?? '', { scaffold: input.scaffold },
          );
          return respond({ ...result });
        }
        case 'approve': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const result = await approvePlan(services, feature);
          const hint = buildTransitionHint('plan_approve');
          return respond({ ...result, ...(hint && { transition: hint }) });
        }
        case 'revoke': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          if (!services.planAdapter.isApproved(feature)) {
            throw new MaestroError(`Plan for '${feature}' is not approved`, [
              'Only approved plans can be revoked',
            ]);
          }
          const allTasks = await services.taskPort.list(feature, { includeAll: true });
          const activeTasks = allTasks.filter(t =>
            t.status === 'claimed' || t.status === 'review' || t.status === 'revision',
          );
          if (activeTasks.length > 0) {
            const activeList = activeTasks.map(t => `${t.id} [${t.status}]`).join(', ');
            throw new MaestroError(
              `Cannot revoke: ${activeTasks.length} task(s) are actively being worked`,
              [`Active tasks: ${activeList}`, 'Wait for active tasks to complete or block them first'],
            );
          }
          services.planAdapter.revokeApproval(feature);
          services.featureAdapter.updateStatus(feature, 'planning');
          return respond({ feature, revoked: true });
        }
        case 'comment': {
          if (!input.body) return respond({ error: 'body is required for action: comment' });
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const comment = services.planAdapter.addComment(feature, {
            body: input.body,
            line: input.line ?? 0,
            author: input.author ?? 'agent',
          });
          return respond({ feature, comment });
        }
        case 'comments_clear': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          services.planAdapter.clearComments(feature);
          return respond({ feature, cleared: true });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: plan_read stays as its own tool per spec
  server.registerTool(
    'maestro_plan_read',
    {
      description: 'Read the plan and any review comments for a feature.',
      inputSchema: {
        feature: featureParam(),
        summary: z.boolean().optional().default(false).describe('Return outline only (preview, headings, commentCount)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);

      const plan = services.planAdapter.read(feature);
      if (!plan) {
        throw new MaestroError(`No plan found for feature '${feature}'`, ['Write a plan with maestro_plan action: write']);
      }

      if (input.summary) {
        const { preview, headings } = extractPlanOutline(plan.content);
        return respond({
          feature,
          plan: { preview, headings, status: plan.status, commentCount: plan.comments.length },
        });
      }

      return respond({ feature, plan });
    }),
  );
}
