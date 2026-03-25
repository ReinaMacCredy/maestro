/**
 * maestro memory-archive -- archive all memory files for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'memory-archive', description: 'Archive memory files' },
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
      const result = memoryAdapter.archive(args.feature);
      output(result, (r) => {
        if (r.archived.length === 0) return 'No memory files to archive.';
        return `[ok] archived ${r.archived.length} file(s) --> ${r.archivePath}\n` +
          r.archived.map((name: string) => `  - ${name}`).join('\n');
      });
    } catch (err) {
      handleCommandError('memory-archive', err);
    }
  },
});
