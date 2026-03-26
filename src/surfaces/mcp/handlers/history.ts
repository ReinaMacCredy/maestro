import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { limitParam } from '../params.ts';
import { history } from '../../../app/workflow/history.ts';
import type { FeatureStatusType } from '../../../domain/types.ts';

const FEATURE_STATUSES = ['planning', 'approved', 'executing', 'completed'] as const;

export function registerHistoryTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_history',
    {
      description: 'Browse completed features with task counts and durations. Use to review past work, check how long features took, or find patterns across completed tracks.',
      inputSchema: {
        limit: limitParam(10),
        status: z.enum(FEATURE_STATUSES).optional().describe('Filter by feature status'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input: { limit?: number; status?: FeatureStatusType }) => {
      const services = thunk.get();
      const result = await history(services, {
        limit: input.limit,
        status: input.status,
      });
      return respond({ ...result });
    }),
  );
}
