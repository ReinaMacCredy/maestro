/**
 * maestro doctrine-approve -- approve a doctrine suggestion.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireDoctrinePort, parseTags } from '../../../../infra/utils/resolve.ts';
import { buildDoctrineItem } from '../../../../app/doctrine/factory.ts';

export default defineCommand({
  meta: { name: 'doctrine-approve', description: 'Approve a doctrine suggestion\n\nExamples:\n  maestro doctrine-approve --name no-mocks --rule "Never mock internal modules" --rationale "Mocks hide bugs"\n  maestro doctrine-approve --name no-mocks --rule "..." --rationale "..." --tags "testing,quality"' },
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
