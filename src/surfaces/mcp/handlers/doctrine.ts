import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
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
        'Doctrine mutations. Actions: write (create/update doctrine item), approve (approve suggestion -> active), ' +
        'suggest (analyze patterns to find candidates), deprecate (mark item inactive).',
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
          if (!input.name) return respond({ error: 'name is required for action: write' });
          if (!input.rule) return respond({ error: 'rule is required for action: write' });
          if (!input.rationale) return respond({ error: 'rationale is required for action: write' });
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
          if (!input.name) return respond({ error: 'name is required for action: approve' });
          if (!input.rule) return respond({ error: 'rule is required for action: approve' });
          if (!input.rationale) return respond({ error: 'rationale is required for action: approve' });
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
          if (!input.name) return respond({ error: 'name is required for action: deprecate' });
          const port = requireDoctrinePort(thunk.get());
          const item = port.deprecate(input.name);
          return respond({ name: item.name, status: item.status });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: list | read
  server.registerTool(
    'maestro_doctrine_read',
    {
      description: 'Doctrine read operations. What: list (all items, optionally filtered by status), read (single item by name).',
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
          if (!input.name) return respond({ error: 'name is required for what: read' });
          const item = port.read(input.name);
          if (!item) {
            throw new MaestroError(`Doctrine item '${input.name}' not found`, ['Use maestro_doctrine_read with what: list to see available items']);
          }
          return respond(item as unknown as Record<string, unknown>);
        }
        default:
          return respond({ error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}
