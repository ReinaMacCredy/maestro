import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING, ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';
import { completeFeature } from '../../../app/features/complete-feature.ts';
import { buildTransitionHint } from '../../../app/workflow/playbook.ts';
import { MaestroError } from '../../../domain/errors.ts';

export function registerFeatureTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: create | complete
  server.registerTool(
    'maestro_feature',
    {
      description:
        'Start a new feature track or mark one as complete. Use when beginning work on a new feature, bug fix, or chore, ' +
        'or when all tasks are done and the feature is ready to close. ' +
        'Actions: create (requires: name), complete. ' +
        'Example: {action: "create", name: "auth-refactor"}',
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
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: create', suggestions: ['Provide the name parameter.'] });
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
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: list | info | active
  server.registerTool(
    'maestro_feature_read',
    {
      description:
        'List features, check which feature is active, or get details on a specific feature. ' +
        'Use to see all tracked features or check the current active feature context. ' +
        'What: list, info (requires: feature name), active (returns the current feature). ' +
        'Example: {what: "active"}',
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
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
