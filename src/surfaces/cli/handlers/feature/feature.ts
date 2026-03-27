/**
 * maestro feature-active -- show or set the active feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { renderStatusLine } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'feature-active', description: 'Show or set active feature\n\nExamples:\n  maestro feature-active\n  maestro feature-active --json' },
  args: {
    name: {
      type: 'positional',
      description: 'Feature name to inspect',
      required: false,
    },
  },
  async run({ args }) {
    try {
      const { featureAdapter } = getServices();

      if (args.name) {
        const feature = featureAdapter.get(args.name);
        if (!feature) {
          throw new MaestroError(
            `Feature '${args.name}' not found`,
            [`Run 'maestro feature-list' to see available features`],
          );
        }
        if (feature.status === 'completed') {
          throw new MaestroError(
            `Feature '${args.name}' is completed`,
            [`Use --feature ${args.name} with commands to inspect it, or create a new feature`],
          );
        }
        output(feature, (f) => [
          renderStatusLine('Active feature', f.name),
          renderStatusLine('Status', f.status),
        ].join('\n'));
        return;
      }

      const active = featureAdapter.getActive();

      output(active, (f) => {
        if (!f) return 'No active feature.';
        return [
          renderStatusLine('Active feature', f.name),
          renderStatusLine('Status', f.status),
        ].join('\n');
      });
    } catch (err) {
      handleCommandError('feature-active', err);
    }
  },
});
