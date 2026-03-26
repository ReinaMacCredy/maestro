/**
 * MCP tool for querying the execution knowledge graph.
 */

import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam, limitParam } from '../params.ts';
import { executionInsights } from '../../../app/workflow/insights.ts';

export function registerExecutionInsightsTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_execution_insights',
    {
      description:
        'Trace how execution knowledge flows through task dependencies. Use to understand which tasks ' +
        'generated knowledge, what downstream tasks benefit, and where coverage gaps exist. ' +
        'Shows exec-* memory details, coverage stats, and knowledge flow edges. ' +
        'Example: maestro_execution_insights({ feature: "auth-refactor" }) ' +
        'Returns: { memories[], coverageStats, knowledgeFlowEdges[], gaps[] }',
      inputSchema: {
        feature: featureParam(),
        limit: limitParam(50),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();
      const feature = requireFeature(services, input.feature);
      const result = await executionInsights(feature, services.taskPort, services.memoryAdapter, services.doctrinePort);
      const data = result as unknown as Record<string, unknown>;
      // Apply limit to array fields to prevent unbounded responses
      if (input.limit && Array.isArray(data.memories)) {
        data.memories = (data.memories as unknown[]).slice(0, input.limit);
      }
      return respond(data);
    }),
  );
}
