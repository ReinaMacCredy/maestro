/**
 * maestro plan-write -- write/update feature plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { writePlan } from '../../../../app/plans/write-plan.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import * as fs from 'fs';

export default defineCommand({
  meta: { name: 'plan-write', description: 'Write or update feature plan' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    content: {
      type: 'string',
      description: 'Plan content (or use --file)',
    },
    file: {
      type: 'string',
      description: 'Read plan content from file',
    },
    scaffold: {
      type: 'boolean',
      description: 'Write a plan template scaffold instead of real content',
      default: false,
    },
    'dry-run': {
      type: 'boolean',
      description: 'Preview plan write without modifying files',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();

      const dryRun = args['dry-run'];

      if (args.scaffold) {
        const result = await writePlan(services, args.feature, '', { scaffold: true, dryRun });
        output(result, (r) => `[ok] plan scaffold written for '${r.feature}'${dryRun ? ' (dry run)' : ''} -- edit ${r.path} then run plan-write with content`);
        return;
      }

      let content = args.content;
      if (!content && args.file) {
        content = fs.readFileSync(args.file, 'utf-8');
      }
      if (!content) {
        throw new MaestroError('No content provided', [
          'Pass --content "..." or --file path/to/plan.md or --scaffold',
        ]);
      }

      const result = await writePlan(services, args.feature, content, { dryRun });
      output(result, (r) => `[ok] plan written for '${r.feature}' (${r.taskCount} task headings)${dryRun ? ' (dry run)' : ''}`);
    } catch (err) {
      handleCommandError('plan-write', err);
    }
  },
});
