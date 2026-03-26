/**
 * maestro plan-comments-clear -- clear all plan comments.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'plan-comments-clear', description: 'Clear all plan comments\n\nExamples:\n  maestro plan-comments-clear --feature my-feat\n  maestro plan-comments-clear --feature my-feat --json' },
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
      planAdapter.clearComments(args.feature);

      output({ feature: args.feature }, () => `[ok] comments cleared`);
    } catch (err) {
      handleCommandError('plan-comments-clear', err);
    }
  },
});
