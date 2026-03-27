/**
 * maestro memory-read -- read a memory file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'memory-read', description: 'Read a memory file\n\nExamples:\n  maestro memory-read --feature my-feat --name finding\n  maestro memory-read --feature my-feat --name finding --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    name: {
      type: 'string',
      description: 'Memory file name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);
      const { memoryAdapter } = services;
      const content = memoryAdapter.read(featureName, args.name);
      if (content === null) {
        throw new MaestroError(`memory '${args.name}' not found for feature '${featureName}'`);
      }
      output({ name: args.name, content }, (c) => c.content);
    } catch (err) {
      handleCommandError('memory-read', err);
    }
  },
});
