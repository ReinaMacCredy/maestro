/**
 * maestro plan-read -- read feature plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError, MaestroError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'plan-read', description: 'Read feature plan' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const { planAdapter } = getServices();
      const result = planAdapter.read(args.feature);

      if (!result) {
        throw new MaestroError(`No plan found for feature '${args.feature}'`, [
          'Write a plan first: maestro plan-write --feature <name> --content "..."',
        ]);
      }

      output(result, (r) => {
        const lines = [
          `status: ${r.status}`,
          `comments: ${r.comments.length}`,
          '',
          r.content,
        ];
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('plan-read', err);
    }
  },
});
