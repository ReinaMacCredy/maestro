/**
 * maestro feature-complete -- mark feature as completed.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { completeFeature } from '../../../../app/features/complete-feature.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'feature-complete', description: 'Mark feature as completed\n\nExamples:\n  maestro feature-complete --feature my-feature\n  maestro feature-complete --feature my-feature --dry-run' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    'dry-run': {
      type: 'boolean',
      description: 'Preview completion without modifying feature',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const result = await completeFeature(services, args.feature, { dryRun: args['dry-run'] });

      output(result, (r) => {
        const { total, done } = r.tasksSummary;
        const suffix = args['dry-run'] ? ' (dry run)' : '';
        return `[ok] feature '${args.feature}' completed (${done}/${total} done)${suffix}`;
      });
    } catch (err) {
      handleCommandError('feature-complete', err);
    }
  },
});
