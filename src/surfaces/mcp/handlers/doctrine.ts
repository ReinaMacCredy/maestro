import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_READONLY, ANNOTATIONS_MUTATING } from '../annotations.ts';
import { requireDoctrinePort } from '../../../infra/utils/resolve.ts';
import { buildDoctrineItem } from '../../../app/doctrine/factory.ts';
import { MaestroError } from '../../../domain/errors.ts';
import { suggestDoctrine } from '../../../app/doctrine/suggest.ts';

export function registerDoctrineTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: write | approve | suggest | deprecate
  server.registerTool(
    'maestro_doctrine',
    {
      description:
        'Create or manage cross-feature operating rules learned from execution. Use after completing a feature ' +
        'to codify patterns worth repeating, or to approve/deprecate suggested rules. ' +
        'Actions: write (requires: name, rule, rationale), approve (requires: name, rule, rationale), ' +
        'suggest (auto-detect candidates from execution patterns), deprecate (requires: name). ' +
        'Example: maestro_doctrine({ action: "write", name: "no-any", rule: "Avoid any type", rationale: "Type safety" })',
      inputSchema: {
        action: z.enum(['write', 'approve', 'suggest', 'deprecate']).describe('Action to perform'),
        name: z.string().optional().describe('Doctrine item name in kebab-case (required for write, approve, deprecate)'),
        rule: z.string().optional().describe('The operating rule -- what to do (required for write, approve)'),
        rationale: z.string().optional().describe('Why this rule exists (required for write, approve)'),
        tags: z.array(z.string()).optional().describe('Tags for relevance matching'),
        conditionTags: z.array(z.string()).optional().describe('Tags that trigger this doctrine'),
        conditionFilePatterns: z.array(z.string()).optional().describe('File glob patterns that trigger this doctrine (write only)'),
        sourceFeatures: z.array(z.string()).optional().describe('Features that informed this doctrine'),
        sourceMemories: z.array(z.string()).optional().describe('Execution memories that informed this doctrine'),
        status: z.enum(['active', 'deprecated', 'proposed']).optional().default('active').describe('Doctrine status (write only)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'write': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: write', suggestions: ['Provide the name parameter.'] });
          if (!input.rule) return errorResponse({ terminal: false, reason: 'validation', error: 'rule is required for action: write', suggestions: ['Provide the rule parameter.'] });
          if (!input.rationale) return errorResponse({ terminal: false, reason: 'validation', error: 'rationale is required for action: write', suggestions: ['Provide the rationale parameter.'] });
          const port = requireDoctrinePort(thunk.get());
          const existing = port.read(input.name) ?? undefined;
          const item = buildDoctrineItem({
            name: input.name,
            rule: input.rule,
            rationale: input.rationale,
            tags: input.tags,
            conditionTags: input.conditionTags,
            conditionFilePatterns: input.conditionFilePatterns,
            sourceFeatures: input.sourceFeatures,
            sourceMemories: input.sourceMemories,
            status: input.status,
            existing,
          });
          const path = port.write(item);
          return respond({ name: item.name, path, created: !existing });
        }
        case 'approve': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: approve', suggestions: ['Provide the name parameter.'] });
          if (!input.rule) return errorResponse({ terminal: false, reason: 'validation', error: 'rule is required for action: approve', suggestions: ['Provide the rule parameter.'] });
          if (!input.rationale) return errorResponse({ terminal: false, reason: 'validation', error: 'rationale is required for action: approve', suggestions: ['Provide the rationale parameter.'] });
          const port = requireDoctrinePort(thunk.get());
          const item = buildDoctrineItem({
            name: input.name,
            rule: input.rule,
            rationale: input.rationale,
            tags: input.tags,
            conditionTags: input.conditionTags,
            sourceFeatures: input.sourceFeatures,
            sourceMemories: input.sourceMemories,
          });
          const path = port.write(item);
          return respond({ name: item.name, path, approved: true });
        }
        case 'suggest': {
          const services = thunk.get();
          const port = requireDoctrinePort(thunk.get());
          const existing = port.list({ status: 'active' });
          const config = services.settingsPort.get().doctrine;
          const result = suggestDoctrine(services.featureAdapter, services.memoryAdapter, existing, config);
          return respond(result as unknown as Record<string, unknown>);
        }
        case 'deprecate': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: deprecate', suggestions: ['Provide the name parameter.'] });
          const port = requireDoctrinePort(thunk.get());
          const item = port.deprecate(input.name);
          return respond({ name: item.name, status: item.status });
        }
        default:
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: list | read
  server.registerTool(
    'maestro_doctrine_read',
    {
      description:
        'Read active operating rules or browse all doctrine items. Use to check existing rules ' +
        'before writing new ones, or to review what has been learned from past features. ' +
        'What: list (optionally filtered by status), read (requires: name). ' +
        'Example: maestro_doctrine_read({ what: "list" })',
      inputSchema: {
        what: z.enum(['list', 'read']).describe('What to read'),
        name: z.string().optional().describe('Doctrine item name (required for what: read)'),
        status: z.enum(['active', 'deprecated', 'proposed']).optional().describe('Filter by status (list only)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const port = requireDoctrinePort(thunk.get());
      switch (input.what) {
        case 'list': {
          const items = port.list(input.status ? { status: input.status } : undefined);
          return respond({
            count: items.length,
            items: items.map(i => ({
              name: i.name,
              rule: i.rule,
              status: i.status,
              tags: i.tags,
              effectiveness: i.effectiveness,
            })),
          });
        }
        case 'read': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for what: read', suggestions: ['Provide the name parameter.'] });
          const item = port.read(input.name);
          if (!item) {
            throw new MaestroError(`Doctrine item '${input.name}' not found`, ['Use maestro_doctrine_read with what: list to see available items']);
          }
          return respond(item as unknown as Record<string, unknown>);
        }
        default:
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
