/**
 * maestro doctrine-read -- read a doctrine item.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import { requireDoctrinePort } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'doctrine-read', description: 'Read a doctrine item\n\nExamples:\n  maestro doctrine-read --name no-mocks\n  maestro doctrine-read --name no-mocks --json' },
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
      const item = doctrinePort.read(args.name);
      if (!item) {
        throw new MaestroError(`doctrine '${args.name}' not found`);
      }
      output(item, (i) => [
        `Name: ${i.name}`,
        `Status: ${i.status}`,
        `Rule: ${i.rule}`,
        `Rationale: ${i.rationale}`,
        `Tags: ${i.tags.join(', ') || '(none)'}`,
        `Injections: ${i.effectiveness.injectionCount}`,
        `Success rate: ${(i.effectiveness.associatedSuccessRate * 100).toFixed(0)}%`,
      ].join('\n'));
    } catch (err) {
      handleCommandError('doctrine-read', err);
    }
  },
});
