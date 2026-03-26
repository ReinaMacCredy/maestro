import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { visualize } from '../../../app/visual/visualize.ts';
import type { MaestroVisualType, VisualResult } from '../../../app/visual/types.ts';
import { MAESTRO_VISUAL_TYPES } from '../../../app/visual/types.ts';
import { output } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../infra/utils/resolve.ts';

function formatResult(result: VisualResult): string {
  const lines: string[] = [];
  lines.push(`[ok] Generated ${result.type} visualization`);
  if (result.feature) lines.push(`Feature: ${result.feature}`);
  lines.push(`Path: ${result.path}`);
  lines.push(result.opened ? 'Opened in browser.' : 'Browser not opened (use without --no-open to auto-open).');
  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'visual', description: 'Render maestro state as interactive HTML\n\nExamples:\n  maestro visual --type status-dashboard --feature my-feat\n  maestro visual --type dependency-graph --feature my-feat --no-open' },
  args: {
    type: {
      type: 'string',
      required: true,
      description: `Visualization type: ${MAESTRO_VISUAL_TYPES.join(', ')}`,
    },
    feature: {
      type: 'string',
      description: 'Feature name (uses active feature if omitted)',
    },
    'no-open': {
      type: 'boolean',
      default: false,
      description: 'Do not open browser automatically',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      if (!MAESTRO_VISUAL_TYPES.includes(args.type as MaestroVisualType)) {
        throw new Error(`Invalid type: ${args.type}. Valid: ${MAESTRO_VISUAL_TYPES.join(', ')}`);
      }

      const result = await visualize(
        args.type as MaestroVisualType,
        featureName,
        services,
        !args['no-open'],
      );
      output(result, formatResult);
    } catch (err) {
      handleCommandError('visual', err);
    }
  },
});
