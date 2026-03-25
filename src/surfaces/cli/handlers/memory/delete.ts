/**
 * maestro memory-delete -- delete a memory file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { MaestroError, handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'memory-delete', description: 'Delete a memory file' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (required unless --global)',
    },
    name: {
      type: 'string',
      description: 'Memory file name',
      required: true,
    },
    global: {
      type: 'boolean',
      description: 'Delete from global project memory',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter } = getServices();

      let deleted: boolean;
      if (args.global) {
        deleted = memoryAdapter.deleteGlobal(args.name);
      } else {
        if (!args.feature) {
          throw new MaestroError('Missing --feature (required unless --global)');
        }
        deleted = memoryAdapter.delete(args.feature, args.name);
      }

      if (!deleted) {
        const scope = args.global ? 'global memory' : `feature '${args.feature}'`;
        throw new MaestroError(`memory '${args.name}' not found in ${scope}`);
      }
      output(deleted, () => `[ok] memory '${args.name}' deleted`);
    } catch (err) {
      handleCommandError('memory-delete', err);
    }
  },
});
