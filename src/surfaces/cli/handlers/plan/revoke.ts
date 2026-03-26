/**
 * maestro plan-revoke -- revoke plan approval.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { revokePlan } from '../../../../app/plans/revoke-plan.ts';

export default defineCommand({
  meta: { name: 'plan-revoke', description: 'Revoke plan approval\n\nExamples:\n  maestro plan-revoke --feature my-feat\n  maestro plan-revoke --feature my-feat --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      await revokePlan(services, args.feature);
      output({ feature: args.feature, revoked: true }, () => `[ok] plan approval revoked`);
    } catch (err) {
      handleCommandError('plan-revoke', err);
    }
  },
});
