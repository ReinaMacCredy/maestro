/**
 * maestro feature-complete -- mark feature as completed.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { completeFeature } from '../../../../app/features/complete-feature.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'feature-complete', description: 'Mark feature as completed\n\nExamples:\n  maestro feature-complete --feature my-feature\n  maestro feature-complete --feature my-feature --dry-run' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
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
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);
      const result = await completeFeature(services, featureName, { dryRun: args['dry-run'] });

      const enriched = { ...result, completedFeature: featureName, hint: `Feature '${featureName}' completed. Use --feature ${featureName} with subsequent commands.` };
      output(enriched, (r) => {
        const { total, done } = r.tasksSummary;
        const suffix = args['dry-run'] ? ' (dry run)' : '';
        return `[ok] feature '${featureName}' completed (${done}/${total} done)${suffix}\n[hint] Use --feature ${featureName} with subsequent commands (no active feature set).`;
      });
    } catch (err) {
      handleCommandError('feature-complete', err);
    }
  },
});
