/**
 * maestro memory-promote -- promote feature memory to global.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { MaestroError, handleCommandError } from '../../../../core/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'memory-promote', description: 'Promote feature memory to global' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    name: {
      type: 'string',
      description: 'Memory file name to promote',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const content = services.memoryAdapter.read(featureName, args.name);
      if (!content) {
        throw new MaestroError(`memory '${args.name}' not found in feature '${featureName}'`);
      }

      const promotedTo = services.memoryAdapter.writeGlobal(args.name, content);

      output({ feature: featureName, name: args.name, promotedTo }, () =>
        `[ok] promoted '${args.name}' to global memory`,
      );
    } catch (err) {
      handleCommandError('memory-promote', err);
    }
  },
});
