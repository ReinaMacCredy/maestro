/**
 * maestro doctrine-write -- create or update a doctrine item.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import { requireDoctrinePort, parseTags } from '../../../../infra/utils/resolve.ts';
import { buildDoctrineItem } from '../../../../app/doctrine/factory.ts';
import type { DoctrineStatus } from '../../../../domain/ports/doctrine.ts';

const VALID_STATUSES = new Set<DoctrineStatus>(['active', 'deprecated', 'proposed']);

export default defineCommand({
  meta: { name: 'doctrine-write', description: 'Create or update a doctrine item\n\nExamples:\n  maestro doctrine-write --name no-mocks --rule "Never mock internal modules" --rationale "Mocks hide integration bugs"\n  maestro doctrine-write --name no-mocks --stdin --rationale "Mocks hide integration bugs"' },
  args: {
    name: { type: 'string', description: 'Doctrine item name (kebab-case)', required: true },
    rule: { type: 'string', description: 'The operating rule (or use --stdin)' },
    rationale: { type: 'string', description: 'Why this rule exists', required: true },
    stdin: { type: 'boolean', description: 'Read rule text from stdin', default: false },
    tags: { type: 'string', description: 'Comma-separated tags' },
    status: { type: 'string', description: 'Status: active, deprecated, proposed', default: 'active' },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const doctrinePort = requireDoctrinePort(services);

      let rule = args.rule;
      if (!rule && args.stdin) {
        rule = await readStdinText();
      }
      if (!rule) {
        throw new MaestroError('No rule provided', [
          'Pass --rule "..." or --stdin',
        ]);
      }

      const existing = doctrinePort.read(args.name) ?? undefined;
      const tags = parseTags(args.tags);
      const status = VALID_STATUSES.has(args.status as DoctrineStatus)
        ? (args.status as DoctrineStatus)
        : 'active';

      const item = buildDoctrineItem({
        name: args.name,
        rule,
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
