/**
 * maestro doctrine-deprecate -- deprecate a doctrine item.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireDoctrinePort } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'doctrine-deprecate', description: 'Deprecate a doctrine item' },
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
