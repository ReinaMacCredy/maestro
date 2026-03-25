import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireFeature } from '../../../core/resolve.ts';
import { featureParam } from '../params.ts';
import { visualize } from '../../../visual/visualize.ts';
import { debugVisualize } from '../../../visual/debug-visualize.ts';

export function registerVisualTools(server: McpServer, thunk: ServicesThunk): void {
  server.registerTool(
    'maestro_visual',
    {
      description:
        'Render HTML visualizations. Type selects the visualization kind. ' +
        'Maestro state types: plan-graph (task dependency flowchart), status-dashboard (KPI cards + progress), ' +
        'memory-map (category grid + distribution), execution-timeline (task events + knowledge flow), ' +
        'doctrine-network (doctrine relationships + effectiveness). ' +
        'Debug types (require data): component-tree, state-flow, error-cascade, network-waterfall, dom-diff, console-timeline. ' +
        'Writes to ~/.maestro/visuals/.',
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
        if (!input.data) return respond({ error: 'data is required for debug visualization types' });
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
