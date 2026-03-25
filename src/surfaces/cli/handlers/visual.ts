import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { visualize } from '../../../visual/visualize.ts';
import type { MaestroVisualType, VisualResult } from '../../../visual/types.ts';
import { MAESTRO_VISUAL_TYPES } from '../../../visual/types.ts';
import { output } from '../../../core/output.ts';
import { handleCommandError } from '../../../core/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../core/resolve.ts';

function formatResult(result: VisualResult): string {
  const lines: string[] = [];
  lines.push(`[ok] Generated ${result.type} visualization`);
  if (result.feature) lines.push(`Feature: ${result.feature}`);
  lines.push(`Path: ${result.path}`);
  lines.push(result.opened ? 'Opened in browser.' : 'Browser not opened (use without --no-open to auto-open).');
  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'visual', description: 'Render maestro state as interactive HTML' },
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
