/**
 * maestro memory-delete -- delete a memory file.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'memory-delete', description: 'Delete a memory file\n\nExamples:\n  maestro memory-delete --feature my-feat --name old-finding\n  maestro memory-delete --feature my-feat --name stale-notes --dry-run' },
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
    'dry-run': {
      type: 'boolean',
      description: 'Preview deletion without removing file',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const { memoryAdapter } = getServices();

      if (args['dry-run']) {
        // Check existence without deleting
        let exists: boolean;
        if (args.global) {
          exists = memoryAdapter.readGlobal(args.name) != null;
        } else {
          if (!args.feature) {
            throw new MaestroError('Missing --feature (required unless --global)');
          }
          exists = memoryAdapter.read(args.feature, args.name) != null;
        }
        if (!exists) {
          const scope = args.global ? 'global memory' : `feature '${args.feature}'`;
          throw new MaestroError(`memory '${args.name}' not found in ${scope}`);
        }
        output({ name: args.name, feature: args.feature, wouldDelete: true }, () =>
          `[ok] memory '${args.name}' would be deleted (dry run)`);
        return;
      }

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
