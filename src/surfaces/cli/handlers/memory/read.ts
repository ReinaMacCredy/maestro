/**
 * maestro memory-read -- read a memory file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'memory-read', description: 'Read a memory file\n\nExamples:\n  maestro memory-read --feature my-feat --name finding\n  maestro memory-read --feature my-feat --name finding --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    name: {
      type: 'string',
      description: 'Memory file name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter } = getServices();
      const content = memoryAdapter.read(args.feature, args.name);
      if (content === null) {
        throw new MaestroError(`memory '${args.name}' not found for feature '${args.feature}'`);
      }
      output(content, (c) => c);
    } catch (err) {
      handleCommandError('memory-read', err);
    }
  },
});
