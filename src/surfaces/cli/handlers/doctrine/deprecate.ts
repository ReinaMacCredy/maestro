/**
 * maestro doctrine-deprecate -- deprecate a doctrine item.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireDoctrinePort } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'doctrine-deprecate', description: 'Deprecate a doctrine item\n\nExamples:\n  maestro doctrine-deprecate --name old-rule\n  maestro doctrine-deprecate --name old-rule --json' },
  args: {
    name: {
      type: 'string',
      description: 'Doctrine item name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const doctrinePort = requireDoctrinePort(services);
      const item = doctrinePort.deprecate(args.name);
      output({ name: item.name, status: item.status }, () =>
        `[ok] doctrine '${item.name}' deprecated`,
      );
    } catch (err) {
      handleCommandError('doctrine-deprecate', err);
    }
  },
});
