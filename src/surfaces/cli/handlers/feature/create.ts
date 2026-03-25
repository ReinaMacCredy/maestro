/**
 * maestro feature-create -- create a new feature.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'feature-create', description: 'Create a new feature' },
  args: {
    name: {
      type: 'positional',
      description: 'Feature name',
      required: true,
    },
    ticket: {
      type: 'string',
      description: 'Associated ticket ID',
    },
  },
  async run({ args }) {
    try {
      const { featureAdapter } = getServices();
      const feature = featureAdapter.create(args.name, args.ticket);
      output(feature, (f) => `[ok] feature '${f.name}' created [${f.status}]`);
    } catch (err) {
      handleCommandError('feature-create', err);
    }
  },
});
