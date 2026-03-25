/**
 * MCP tools for bv graph intelligence.
 * Exposes dependency graph analysis, next-bead recommendation,
 * parallel execution planning, parallel task discovery, and batch reservation.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireGraphPort, requireFeature } from '../../../core/resolve.ts';
import { featureParam } from '../params.ts';

export function registerGraphTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_graph',
    {
      description:
        'Graph and parallel execution operations. Actions: insights (bottlenecks, critical path, velocity -- requires bv), ' +
        'next (top recommended next bead with rationale -- requires bv), ' +
        'plan (parallel execution tracks for N agents -- requires bv), ' +
        'discovery (all runnable tasks with specs for parallel dispatch), ' +
        'reserve (batch claim multiple tasks for parallel agents).',
      inputSchema: {
        action: z.enum(['insights', 'next', 'plan', 'discovery', 'reserve'])
          .describe('Action to perform'),
        feature: featureParam(),
        agents: z.number().optional().default(1).describe('Number of parallel agents (plan only)'),
        tasks: z.array(z.string()).optional().describe('Task IDs to claim (reserve only)'),
      },
      // reserve mutates; use ANNOTATIONS_MUTATING to cover all actions conservatively
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'insights': {
          const port = requireGraphPort(thunk.get());
          const insights = await port.getInsights();
          return respond({ ...insights });
        }
        case 'next': {
          const port = requireGraphPort(thunk.get());
          const recommendation = await port.getNextRecommendation();
          if (!recommendation) {
            return respond({ message: 'No recommendations available (all beads may be closed)' });
          }
          return respond({ ...recommendation });
        }
        case 'plan': {
          const port = requireGraphPort(thunk.get());
          const plan = await port.getExecutionPlan(input.agents);
          return respond({ ...plan });
        }
        case 'discovery': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const runnable = await services.taskPort.getRunnable(feature);
          const tasks = await Promise.all(
            runnable.map(async (task) => {
              const spec = await services.taskPort.readSpec(feature, task.id);
              return { id: task.id, name: task.name, status: task.status, dependsOn: task.dependsOn, spec };
            }),
          );
          return respond({ feature, count: tasks.length, tasks });
        }
        case 'reserve': {
          if (!input.tasks || input.tasks.length === 0) return respond({ error: 'tasks array is required for action: reserve' });
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const claimed: string[] = [];
          const failed: Array<{ id: string; reason: string }> = [];
          for (const taskId of input.tasks) {
            try {
              await services.taskPort.claim(feature, taskId, 'parallel-agent');
              claimed.push(taskId);
            } catch (err) {
              failed.push({ id: taskId, reason: err instanceof Error ? err.message : String(err) });
            }
          }
          return respond({ feature, claimed, failed });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );
}

