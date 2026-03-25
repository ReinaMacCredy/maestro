/**
 * maestro memory-stats -- show memory stats for a feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'memory-stats', description: 'Show memory stats' },
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
      const stats = memoryAdapter.stats(args.feature);
      output(stats, (s) => {
        const lines = [
          `files:  ${s.count}`,
          `bytes:  ${s.totalBytes}`,
        ];
        if (s.oldest) lines.push(`oldest: ${s.oldest}`);
        if (s.newest) lines.push(`newest: ${s.newest}`);
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('memory-stats', err);
    }
  },
});
