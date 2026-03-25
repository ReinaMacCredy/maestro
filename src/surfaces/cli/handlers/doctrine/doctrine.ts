/**
 * maestro doctrine-approve -- approve a doctrine suggestion.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireDoctrinePort, parseTags } from '../../../../core/resolve.ts';
import { buildDoctrineItem } from '../../../../doctrine/factory.ts';

export default defineCommand({
  meta: { name: 'doctrine-approve', description: 'Approve a doctrine suggestion' },
  args: {
    name: { type: 'string', description: 'Doctrine item name (kebab-case)', required: true },
    rule: { type: 'string', description: 'The operating rule', required: true },
    rationale: { type: 'string', description: 'Why this rule exists', required: true },
    tags: { type: 'string', description: 'Comma-separated tags' },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const doctrinePort = requireDoctrinePort(services);

      const tags = parseTags(args.tags);
      const item = buildDoctrineItem({
        name: args.name,
        rule: args.rule,
        rationale: args.rationale,
        conditionTags: tags.length > 0 ? tags : undefined,
        tags,
      });

      const path = doctrinePort.write(item);
      output({ name: item.name, path }, () =>
        `[ok] doctrine '${item.name}' approved and saved`,
      );
    } catch (err) {
      handleCommandError('doctrine-approve', err);
    }
  },
});
