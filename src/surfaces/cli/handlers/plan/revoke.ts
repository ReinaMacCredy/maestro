/**
 * maestro plan-revoke -- revoke plan approval.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'plan-revoke', description: 'Revoke plan approval' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const { planAdapter, featureAdapter } = getServices();
      const wasApproved = planAdapter.isApproved(args.feature);
      planAdapter.revokeApproval(args.feature);
      if (wasApproved) {
        featureAdapter.updateStatus(args.feature, 'planning');
      }

      output({ feature: args.feature }, () => `[ok] plan approval revoked`);
    } catch (err) {
      handleCommandError('plan-revoke', err);
    }
  },
});
