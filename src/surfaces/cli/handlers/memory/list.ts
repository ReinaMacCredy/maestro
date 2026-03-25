/**
 * maestro memory-list -- list memory files for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output, renderTable } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'memory-list', description: 'List memory files' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter } = getServices();
      const files = memoryAdapter.listWithMeta(args.feature);
      output(files, (items) => {
        if (items.length === 0) return 'No memory files found.';
        const rows = items.map(f => [
          f.name,
          `${f.content.length} chars`,
          f.metadata.category ?? '-',
          f.metadata.tags?.join(', ') || '-',
          String(f.metadata.priority ?? 2),
          f.updatedAt,
        ]);
        return renderTable(['Name', 'Size', 'Category', 'Tags', 'Pri', 'Updated'], rows);
      });
    } catch (err) {
      handleCommandError('memory-list', err);
    }
  },
});
