/**
 * maestro doctrine-write -- create or update a doctrine item.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireDoctrinePort, parseTags } from '../../../../core/resolve.ts';
import { buildDoctrineItem } from '../../../../doctrine/factory.ts';
import type { DoctrineStatus } from '../../../../doctrine/port.ts';

const VALID_STATUSES = new Set<DoctrineStatus>(['active', 'deprecated', 'proposed']);

export default defineCommand({
  meta: { name: 'doctrine-write', description: 'Create or update a doctrine item' },
  args: {
    name: { type: 'string', description: 'Doctrine item name (kebab-case)', required: true },
    rule: { type: 'string', description: 'The operating rule', required: true },
    rationale: { type: 'string', description: 'Why this rule exists', required: true },
    tags: { type: 'string', description: 'Comma-separated tags' },
    status: { type: 'string', description: 'Status: active, deprecated, proposed', default: 'active' },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const doctrinePort = requireDoctrinePort(services);

      const existing = doctrinePort.read(args.name) ?? undefined;
      const tags = parseTags(args.tags);
      const status = VALID_STATUSES.has(args.status as DoctrineStatus)
        ? (args.status as DoctrineStatus)
        : 'active';

      const item = buildDoctrineItem({
        name: args.name,
        rule: args.rule,
        rationale: args.rationale,
        conditionTags: tags.length > 0 ? tags : undefined,
        tags,
        status,
        existing,
      });

      const path = doctrinePort.write(item);
      output({ name: item.name, path }, () =>
        `[ok] doctrine '${item.name}' ${existing ? 'updated' : 'created'}`,
      );
    } catch (err) {
      handleCommandError('doctrine-write', err);
    }
  },
});
