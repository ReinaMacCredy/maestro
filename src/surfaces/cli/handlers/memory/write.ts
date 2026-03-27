/**
 * maestro memory-write -- write a memory file for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import * as fs from 'fs';
import { parseTags, requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';
import { prependMetadataFrontmatter } from '../../../../infra/utils/frontmatter.ts';
import { MEMORY_CATEGORIES } from '../../../../domain/types.ts';

export default defineCommand({
  meta: { name: 'memory-write', description: 'Write a memory file\n\nExamples:\n  maestro memory-write --feature my-feat --name finding --content "Auth requires OAuth2"\n  maestro memory-write --feature my-feat --name api-notes --file notes.md\n  maestro memory-write --feature my-feat --name api-notes --stdin' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    name: {
      type: 'string',
      description: 'Memory file name',
      required: true,
    },
    content: {
      type: 'string',
      description: 'Content to write (or use --file / --stdin)',
    },
    file: {
      type: 'string',
      description: 'Read content from file',
    },
    stdin: {
      type: 'boolean',
      description: 'Read content from stdin',
      default: false,
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
      const services = getServices();
      const { memoryAdapter } = services;

      let content = args.content;
      if (!content && args.file) {
        content = fs.readFileSync(args.file, 'utf-8');
      }
      if (!content && args.stdin) {
        content = await readStdinText();
      }
      if (!content) {
        throw new MaestroError('No content provided', [
          'Pass --content "..." or --file path/to/file.md or --stdin',
        ]);
      }

      const finalContent = prependMetadataFrontmatter(content, {
        tags: args.tags ? parseTags(args.tags) : undefined,
        priority: args.priority !== undefined ? Number(args.priority) : undefined,
        category: args.category,
      });

      const path = args.global
        ? memoryAdapter.writeGlobal(args.name, finalContent)
        : memoryAdapter.write(requireFeature(services, args.feature, [FEATURE_HINT]), args.name, finalContent);
      output({ name: args.name, path }, (r) => `[ok] memory written --> ${r.path}`);
    } catch (err) {
      handleCommandError('memory-write', err);
    }
  },
});
