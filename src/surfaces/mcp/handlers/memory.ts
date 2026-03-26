import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING, ANNOTATIONS_DESTRUCTIVE, ANNOTATIONS_READONLY } from '../annotations.ts';
import { requireFeature } from '../../../infra/utils/resolve.ts';
import { featureParam } from '../params.ts';
import { MaestroError } from '../../../domain/errors.ts';
import { prependMetadataFrontmatter } from '../../../infra/utils/frontmatter.ts';
import { validateName } from '../../../infra/utils/validate-name.ts';
import { selectMemories } from '../../../app/dcp/selector.ts';
import { resolveDcpConfig } from '../../../app/dcp/config.ts';
import { MEMORY_CATEGORIES, type MemoryRelation } from '../../../domain/types.ts';

export function registerMemoryTools(server: McpServer, thunk: ServicesThunk): void {
  // Mutating: write | delete | promote | compress | consolidate | archive
  server.registerTool(
    'maestro_memory',
    {
      description:
        'Memory mutations. Actions: write (save memory), delete (remove memory), promote (feature -> global), ' +
        'compress (truncate to 200 chars), consolidate (deduplicate + compress + promote), ' +
        'connect (link two memories), archive (archive all to timestamped dir).',
      inputSchema: {
        action: z.enum(['write', 'delete', 'promote', 'compress', 'consolidate', 'connect', 'archive'])
          .describe('Action to perform'),
        feature: z.string().optional().describe('Feature name (defaults to active feature; ignored when global=true)'),
        name: z.string().optional().describe('Memory file name (required for write, delete, promote, compress, connect)'),
        content: z.string().optional().describe('Memory content (required for write)'),
        global: z.boolean().optional().default(false).describe('Target global project memory instead of feature memory (write and delete only)'),
        tags: z.array(z.string()).optional().describe('Tags for DCP relevance scoring (write only)'),
        priority: z.number().min(0).max(4).optional().describe('Priority 0-4 (write only)'),
        category: z.enum(MEMORY_CATEGORIES).optional().describe('Memory category for DCP scoring (write only)'),
        autoPromote: z.boolean().optional().default(false).describe('Auto-promote qualifying memories (consolidate only)'),
        target: z.string().optional().describe('Target memory name (required for connect)'),
        relation: z.enum(['related', 'supersedes', 'contradicts', 'extends']).optional().describe('Relation type (required for connect)'),
      },
      annotations: ANNOTATIONS_DESTRUCTIVE,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();

      switch (input.action) {
        case 'write': {
          if (!input.name) return respond({ error: 'name is required for action: write' });
          if (!input.content) return respond({ error: 'content is required for action: write' });
          const finalContent = prependMetadataFrontmatter(input.content, {
            tags: input.tags, priority: input.priority, category: input.category,
          });
          if (input.global) {
            const path = services.memoryAdapter.writeGlobal(input.name, finalContent);
            return respond({ scope: 'global', name: input.name, path });
          }
          const feature = requireFeature(services, input.feature);
          const path = services.memoryAdapter.write(feature, input.name, finalContent);
          return respond({ feature, name: input.name, path });
        }
        case 'delete': {
          if (!input.name) return respond({ error: 'name is required for action: delete' });
          const validation = validateName(input.name, 'memory name');
          if (!validation.ok) {
            throw new MaestroError(validation.error, ['Provide a valid memory file name']);
          }
          if (input.global) {
            const deleted = services.memoryAdapter.deleteGlobal(validation.name);
            if (!deleted) {
              throw new MaestroError(`Memory '${validation.name}' not found in global memory`, [
                'Use maestro_memory_read with what: list to see available memories',
              ]);
            }
            return respond({ scope: 'global', name: validation.name, deleted: true });
          }
          const feature = requireFeature(services, input.feature);
          const deleted = services.memoryAdapter.delete(feature, validation.name);
          if (!deleted) {
            throw new MaestroError(`Memory '${validation.name}' not found in feature '${feature}'`, [
              'Use maestro_memory_read with what: list to see available memories',
            ]);
          }
          return respond({ feature, name: validation.name, deleted: true });
        }
        case 'promote': {
          if (!input.name) return respond({ error: 'name is required for action: promote' });
          const feature = requireFeature(services, input.feature);
          const content = services.memoryAdapter.read(feature, input.name);
          if (!content) {
            return respond({ success: false, error: `Memory '${input.name}' not found in feature '${feature}'` });
          }
          const path = services.memoryAdapter.writeGlobal(input.name, content);
          return respond({ feature, name: input.name, promotedTo: path });
        }
        case 'compress': {
          if (!input.name) return respond({ error: 'name is required for action: compress' });
          const feature = requireFeature(services, input.feature);
          const success = services.memoryAdapter.compress(feature, input.name);
          if (!success) {
            throw new MaestroError(`Memory '${input.name}' not found in feature '${feature}'`, [
              'Use maestro_memory_read with what: list to see available memories',
            ]);
          }
          return respond({ feature, name: input.name, compressed: true });
        }
        case 'consolidate': {
          const feature = requireFeature(services, input.feature);
          const { consolidateMemories } = await import('../../../app/memory/consolidate.ts');
          const result = consolidateMemories(services.memoryAdapter, feature, { autoPromote: input.autoPromote });
          return respond({ feature, ...result });
        }
        case 'connect': {
          if (!input.name) return respond({ error: 'name is required for action: connect' });
          if (!input.target) return respond({ error: 'target is required for action: connect' });
          if (!input.relation) return respond({ error: 'relation is required for action: connect' });
          const feature = requireFeature(services, input.feature);
          services.memoryAdapter.connect(feature, input.name, input.target, input.relation as MemoryRelation);
          return respond({ feature, source: input.name, target: input.target, relation: input.relation });
        }
        case 'archive': {
          const feature = requireFeature(services, input.feature);
          const result = services.memoryAdapter.archive(feature);
          return respond({ feature, ...result });
        }
        default:
          return respond({ error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );

  // Read-only: read | list | stats | insights | compile
  server.registerTool(
    'maestro_memory_read',
    {
      description:
        'Memory read operations. What: read (single memory file), list (all files, with optional DCP scoring), ' +
        'stats (count/bytes/timestamps), insights (duplicates + compression candidates), compile (concatenate all).',
      inputSchema: {
        what: z.enum(['read', 'list', 'stats', 'insights', 'compile']).describe('What to read'),
        feature: z.string().optional().describe('Feature name (defaults to active feature; omit for global memory)'),
        name: z.string().optional().describe('Memory file name (required for read)'),
        brief: z.boolean().optional().default(false).describe('Return metadata only, omit content (list only)'),
        task: z.string().optional().describe('Task folder for DCP-scored filtering (list only)'),
        budget: z.number().optional().describe('Memory budget in bytes -- default from config (list only)'),
      },
      annotations: ANNOTATIONS_READONLY,
    },
    withErrorHandling(async (input) => {
      const services = thunk.get();

      switch (input.what) {
        case 'read': {
          if (!input.name) return respond({ error: 'name is required for what: read' });
          try {
            const feature = requireFeature(services, input.feature);
            const content = services.memoryAdapter.read(feature, input.name);
            if (content !== null) {
              return respond({ feature, name: input.name, content });
            }
          } catch (err) {
            if (!(err instanceof MaestroError)) throw err;
          }
          const content = services.memoryAdapter.readGlobal(input.name);
          return respond({ scope: 'global', name: input.name, content });
        }
        case 'list': {
          let feature: string;
          try {
            feature = requireFeature(services, input.feature);
          } catch (err) {
            if (!(err instanceof MaestroError)) throw err;
            const globalFiles = services.memoryAdapter.listGlobal();
            const files = input.brief
              ? globalFiles.map(({ name, updatedAt, sizeBytes }) => ({ name, updatedAt, sizeBytes }))
              : globalFiles;
            return respond({ scope: 'global', files });
          }
          const richFiles = services.memoryAdapter.listWithMeta(feature);
          if (input.task) {
            const task = await services.taskPort.get(feature, input.task);
            if (!task) {
              return respond({ error: `Task '${input.task}' not found in feature '${feature}'` });
            }
            const cfg = resolveDcpConfig(services.settingsPort.get().dcp);
            const budget = input.budget ?? cfg.memoryBudgetTokens;
            const featureCreatedAt = services.featureAdapter.get(feature)?.createdAt;
            const selected = selectMemories(
              richFiles, task, task.planTitle ?? null, budget,
              cfg.relevanceThreshold, featureCreatedAt,
            );
            const scoreMap = new Map(selected.scores.map(s => [s.name, s.score]));
            const files = selected.memories.map(m => ({
              name: m.name,
              ...(input.brief ? {} : { content: m.bodyContent }),
              score: scoreMap.get(m.name) ?? 0,
            }));
            return respond({ feature, files, dcp: {
              included: selected.includedCount,
              dropped: selected.droppedCount,
              budgetBytes: budget,
            }});
          }
          const files = input.brief
            ? richFiles.map(({ name, updatedAt, sizeBytes, metadata }) => ({
                name, updatedAt, sizeBytes, ...metadata,
              }))
            : richFiles.map(({ name, content, updatedAt, sizeBytes, metadata }) => ({
                name, content, updatedAt, sizeBytes, ...metadata,
              }));
          return respond({ feature, files });
        }
        case 'stats': {
          const feature = requireFeature(services, input.feature);
          const stats = services.memoryAdapter.stats(feature);
          return respond({ feature, ...stats });
        }
        case 'insights': {
          const feature = requireFeature(services, input.feature);
          const { findDuplicates } = await import('../../../app/dcp/dedup.ts');
          const memories = services.memoryAdapter.listWithMeta(feature);
          const stats = services.memoryAdapter.stats(feature);
          const duplicates = findDuplicates(memories);
          const compressionCandidates = memories
            .filter(m => m.sizeBytes > 2000 && !services.memoryAdapter.isCompressed(feature, m.name))
            .map(m => ({ name: m.name, sizeBytes: m.sizeBytes }));
          return respond({ feature, stats, duplicates, compressionCandidates });
        }
        case 'compile': {
          const feature = requireFeature(services, input.feature);
          const compiled = services.memoryAdapter.compile(feature);
          return respond({ feature, compiled });
        }
        default:
          return respond({ error: `Unknown what: ${(input as { what: string }).what}` });
      }
    }),
  );
}

