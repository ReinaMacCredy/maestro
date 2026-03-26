/**
 * maestro doctrine-list -- list doctrine items.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireDoctrinePort } from '../../../../infra/utils/resolve.ts';
import type { DoctrineStatus } from '../../../../domain/ports/doctrine.ts';

export default defineCommand({
  meta: { name: 'doctrine-list', description: 'List doctrine items\n\nExamples:\n  maestro doctrine-list\n  maestro doctrine-list --status active --json' },
  args: {
    status: {
      type: 'string',
      description: 'Filter by status (active, deprecated, proposed)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const doctrinePort = requireDoctrinePort(services);
      const items = doctrinePort.list(args.status ? { status: args.status as DoctrineStatus } : undefined);
      output(items, (list) => {
        if (list.length === 0) return 'No doctrine items found.';
        return list.map(i =>
          `[${i.status}] ${i.name} -- ${i.rule.slice(0, 60)}${i.rule.length > 60 ? '...' : ''} (injections: ${i.effectiveness.injectionCount})`
        ).join('\n');
      });
    } catch (err) {
      handleCommandError('doctrine-list', err);
    }
  },
});
