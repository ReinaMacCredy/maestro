import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING, ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../core/resolve.ts';
import { featureParam } from '../params.ts';
import { completeFeature } from '../../../features/complete-feature.ts';
import { buildTransitionHint } from '../../../workflow/playbook.ts';
import { MaestroError } from '../../../core/errors.ts';

export function registerFeatureTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: create | complete
  server.registerTool(
    'maestro_feature',
    {
      description: 'Feature lifecycle mutations. Actions: create (new feature), complete (mark done).',
      inputSchema: {
        action: z.enum(['create', 'complete']).describe('Action to perform'),
        feature: featureParam(),
        name: z.string().optional().describe('Feature name (required for create)'),
        ticket: z.string().optional().describe('Ticket reference (optional, create only)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'create': {
          if (!input.name) return respond({ error: 'name is required for action: create' });
          const services = thunk.get();
          const result = services.featureAdapter.create(input.name, input.ticket);
          return respond({ feature: result });
        }
        case 'complete': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const result = await completeFeature(services, feature);
          const hint = buildTransitionHint('feature_complete');
          return respond({ ...result, ...(hint && { transition: hint }) });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: list | info | active
  server.registerTool(
    'maestro_feature_read',
    {
      description: 'Feature read operations. What: list (all features), info (specific feature details), active (current active feature).',
      inputSchema: {
        what: z.enum(['list', 'info', 'active']).describe('What to read'),
        feature: featureParam(),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      switch (input.what) {
        case 'list': {
          const services = thunk.get();
          const features = services.featureAdapter.list();
          const active = services.featureAdapter.getActive(features);
          return respond({ features, active: active?.name ?? null });
        }
        case 'info': {
          const services = thunk.get();
          const feature = requireFeature(services, input.feature);
          const info = services.featureAdapter.getInfo(feature);
          if (!info) {
            throw new MaestroError(`Feature '${feature}' not found`, [
              'Use maestro_feature_read with what: list to see available features',
            ]);
          }
          return respond(info);
        }
        case 'active': {
          const services = thunk.get();
          const active = services.featureAdapter.getActive();
          return respond({ active: active ?? null });
        }
        default:
          return respond({ error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
