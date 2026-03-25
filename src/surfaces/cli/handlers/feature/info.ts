/**
 * maestro feature-info -- show feature details.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output, renderStatusLine } from '../../../../core/output.ts';
import { MaestroError, handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'feature-info', description: 'Show feature details' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const { featureAdapter } = getServices();
      const info = featureAdapter.getInfo(args.feature);

      if (!info) {
        throw new MaestroError(`Feature '${args.feature}' not found`);
      }

      output(info, (i) =>
        [
          renderStatusLine('Name', i.name),
          renderStatusLine('Status', i.status),
          renderStatusLine('Has plan', String(i.hasPlan)),
          renderStatusLine('Comments', String(i.commentCount)),
        ].join('\n'),
      );
    } catch (err) {
      handleCommandError('feature-info', err);
    }
  },
});
