/**
 * maestro plan-comments-clear -- clear all plan comments.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'plan-comments-clear', description: 'Clear all plan comments' },
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
