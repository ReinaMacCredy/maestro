/**
 * MCP tool for querying the execution knowledge graph.
 */

import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';
import { executionInsights } from '../../../app/workflow/insights.ts';

export function registerExecutionInsightsTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_execution_insights',
    {
      description:
        'Query execution knowledge graph: which tasks generated knowledge, how it flows ' +
        'through dependencies, downstream coverage. Shows exec-* memory details, coverage ' +
        'stats, and knowledge flow edges with proximity scores.',
      inputSchema: {
        feature: featureParam(),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);
      const result = await executionInsights(feature, services.taskPort, services.memoryAdapter, services.doctrinePort);
      return respond(result as unknown as Record<string, unknown>);
    }),
  );
}
