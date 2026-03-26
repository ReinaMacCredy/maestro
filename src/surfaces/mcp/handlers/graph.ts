/**
 * MCP tools for bv graph intelligence.
 * Exposes dependency graph analysis, next-bead recommendation,
 * parallel execution planning, parallel task discovery, and batch reservation.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireGraphPort, requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';

export function registerGraphTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_graph',
    {
      description:
        'Analyze task dependencies, find bottlenecks, or plan parallel execution across multiple agents. ' +
        'Use when coordinating multi-agent work, identifying the critical path, or dispatching tasks in parallel. ' +
        'Requires bv integration for insights/next/plan. ' +
        'Actions: insights, next, plan (requires: agents count), discovery (all runnable tasks), reserve (requires: tasks array). ' +
        'Example: maestro_graph({ action: "discovery" })',
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
          if (!input.tasks || input.tasks.length === 0) return errorResponse({ terminal: false, reason: 'validation', error: 'tasks array is required for action: reserve', suggestions: ['Provide the tasks parameter.'] });
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
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );
}

