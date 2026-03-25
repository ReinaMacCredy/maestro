/**
 * maestro plan-comment -- add comment to feature plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'plan-comment', description: 'Add comment to feature plan' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    body: {
      type: 'string',
      description: 'Comment body',
      required: true,
    },
    line: {
      type: 'string',
      description: 'Line number to attach comment to',
    },
    author: {
      type: 'string',
      description: 'Comment author',
    },
  },
  async run({ args }) {
    try {
      const { planAdapter } = getServices();
      const result = planAdapter.addComment(args.feature, {
        body: args.body,
        author: args.author ?? 'cli',
        line: args.line ? Number(args.line) : 0,
      });

      output(result, () => '[ok] comment added to plan');
    } catch (err) {
      handleCommandError('plan-comment', err);
    }
  },
});
