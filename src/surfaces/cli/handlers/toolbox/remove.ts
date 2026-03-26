/**
 * maestro toolbox-remove -- remove an external tool.
 */

import * as fs from 'fs';
import * as path from 'path';
import { defineCommand } from 'citty';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';

const BUILT_IN_TOOLS = new Set(['fs-tasks']);

export default defineCommand({
  meta: { name: 'toolbox-remove', description: 'Remove an external tool\n\nExamples:\n  maestro toolbox-remove --name my-tool\n  maestro toolbox-remove --name my-tool --json' },
  args: {
    name: {
      type: 'string',
      description: 'Tool name to remove',
      required: true,
    },
  },
  async run({ args }) {
    try {
      if (BUILT_IN_TOOLS.has(args.name)) {
        throw new MaestroError(`Cannot remove built-in tool '${args.name}'`);
      }

      const toolDir = path.join(import.meta.dir, '../../../../infra/toolbox/tools/external', args.name);
      if (!fs.existsSync(toolDir)) {
        throw new MaestroError(
          `Tool '${args.name}' not found in external tools`,
          ['Use maestro toolbox-list to see available tools'],
        );
      }

      fs.rmSync(toolDir, { recursive: true, force: true });

      output(
        { name: args.name, removed: true },
        () => `[ok] Removed tool '${args.name}'`,
      );
    } catch (err) {
      handleCommandError('toolbox-remove', err);
    }
  },
});
