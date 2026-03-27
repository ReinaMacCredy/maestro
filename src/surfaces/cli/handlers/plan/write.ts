/**
 * maestro plan-write -- write/update feature plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { writePlan } from '../../../../app/plans/write-plan.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../../domain/errors.ts';
import { readStdinText } from '../../../../infra/utils/stdin.ts';
import * as fs from 'fs';
import { requireFeature, FEATURE_HINT } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'plan-write', description: 'Write or update feature plan\n\nExamples:\n  maestro plan-write --feature my-feat --file plan.md\n  maestro plan-write --feature my-feat --content "## Tasks\\n- Setup auth"' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
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
    stdin: {
      type: 'boolean',
      description: 'Read plan content from stdin',
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
      const featureName = requireFeature(services, args.feature, [FEATURE_HINT]);

      const dryRun = args['dry-run'];

      if (args.scaffold) {
        const result = await writePlan(services, featureName, '', { scaffold: true, dryRun });
        output(result, (r) => `[ok] plan scaffold written for '${r.feature}'${dryRun ? ' (dry run)' : ''} -- edit ${r.path} then run plan-write with content`);
        return;
      }

      let content = args.content;
      if (!content && args.file) {
        content = fs.readFileSync(args.file, 'utf-8');
      }
      if (!content && args.stdin) {
        content = await readStdinText();
      }
      if (!content) {
        throw new MaestroError('No content provided', [
          'Pass --content "..." or --file path/to/plan.md or --stdin or --scaffold',
        ]);
      }

      const result = await writePlan(services, featureName, content, { dryRun });
      output(result, (r) => {
        let msg = `[ok] plan written for '${r.feature}' (${r.taskCount} task headings detected)${dryRun ? ' (dry run)' : ''}`;
        if (r.warnings?.length) {
          msg += '\n' + r.warnings.map((w: string) => `[warn] ${w}`).join('\n');
        }
        return msg;
      });
    } catch (err) {
      handleCommandError('plan-write', err);
    }
  },
});
