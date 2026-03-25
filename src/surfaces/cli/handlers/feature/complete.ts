/**
 * maestro feature-complete -- mark feature as completed.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { completeFeature } from '../../../../features/complete-feature.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'feature-complete', description: 'Mark feature as completed' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const result = await completeFeature(services, args.feature);

      output(result, (r) => {
        const { total, done } = r.tasksSummary;
        return `[ok] feature '${args.feature}' completed (${done}/${total} done)`;
      });
    } catch (err) {
      handleCommandError('feature-complete', err);
    }
  },
});
