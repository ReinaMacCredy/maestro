/**
 * maestro memory-write -- write a memory file for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { parseTags } from '../../../../core/resolve.ts';
import { prependMetadataFrontmatter } from '../../../../core/frontmatter.ts';
import { MEMORY_CATEGORIES } from '../../../../core/types.ts';

export default defineCommand({
  meta: { name: 'memory-write', description: 'Write a memory file' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    name: {
      type: 'string',
      description: 'Memory file name',
      required: true,
    },
    content: {
      type: 'string',
      description: 'Content to write',
      required: true,
    },
    global: {
      type: 'boolean',
      description: 'Write to global project memory instead of feature memory',
      default: false,
    },
    tags: {
      type: 'string',
      description: 'Comma-separated tags for DCP relevance scoring',
    },
    priority: {
      type: 'string',
      description: 'Priority 0 (highest) to 4 (lowest), default 2',
    },
    category: {
      type: 'string',
      description: `Category: ${MEMORY_CATEGORIES.join(', ')}`,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter } = getServices();

      const finalContent = prependMetadataFrontmatter(args.content, {
        tags: args.tags ? parseTags(args.tags) : undefined,
        priority: args.priority !== undefined ? Number(args.priority) : undefined,
        category: args.category,
      });

      const result = args.global
        ? memoryAdapter.writeGlobal(args.name, finalContent)
        : memoryAdapter.write(args.feature, args.name, finalContent);
      output(result, (r) => `[ok] memory written --> ${r}`);
    } catch (err) {
      handleCommandError('memory-write', err);
    }
  },
});
