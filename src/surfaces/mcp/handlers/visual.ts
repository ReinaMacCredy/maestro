import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';
import { visualize } from '../../../app/visual/visualize.ts';
import { debugVisualize } from '../../../app/visual/debug-visualize.ts';

export function registerVisualTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_visual',
    {
      description:
        'Generate interactive HTML visualizations of feature progress, task dependencies, or debug data. ' +
        'Use to visualize task graphs, track execution progress, map memory distribution, or render debug diagrams. ' +
        'Maestro types: plan-graph, status-dashboard, memory-map, execution-timeline, doctrine-network. ' +
        'Debug types (require data): component-tree, state-flow, error-cascade, network-waterfall, dom-diff, console-timeline. ' +
        'Example: maestro_visual({ type: "status-dashboard" })',
      inputSchema: {
        type: z.enum([
          'plan-graph', 'status-dashboard', 'memory-map', 'execution-timeline', 'doctrine-network',
          'component-tree', 'state-flow', 'error-cascade', 'network-waterfall', 'dom-diff', 'console-timeline',
        ]).describe('Visualization type'),
        feature: featureParam(),
        autoOpen: z.boolean().optional().default(true).describe('Open browser automatically'),
        data: z.record(z.unknown()).optional().describe('Structured data for debug visualizations'),
        title: z.string().optional().describe('Page title for debug visualizations'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      const debugTypes = ['component-tree', 'state-flow', 'error-cascade', 'network-waterfall', 'dom-diff', 'console-timeline'];

      if (debugTypes.includes(input.type)) {
        if (!input.data) return errorResponse({ terminal: false, reason: 'validation', error: 'data is required for debug visualization types', suggestions: ['Provide the data parameter.'] });
        const result = await debugVisualize(
          input.type as 'component-tree' | 'state-flow' | 'error-cascade' | 'network-waterfall' | 'dom-diff' | 'console-timeline',
          input.data, input.title, input.autoOpen,
        );
        return respond({ path: result.path, opened: result.opened, type: result.type });
      }

      const services = thunk.get();
      const feature = requireFeature(services, input.feature);
      const result = await visualize(
        input.type as 'plan-graph' | 'status-dashboard' | 'memory-map' | 'execution-timeline' | 'doctrine-network',
        feature, services, input.autoOpen,
      );
      return respond({ path: result.path, opened: result.opened, type: result.type, feature: result.feature });
    }),
  );
}
