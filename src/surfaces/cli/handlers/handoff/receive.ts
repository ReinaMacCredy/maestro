/**
 * maestro handoff-receive -- check for pending handoffs.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { resolveFeature, requireHandoffPort } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'handoff-receive', description: 'Check for pending handoffs' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (defaults to active feature)',
    },
    agentId: {
      type: 'string',
      description: 'Agent identifier to check handoffs for',
      required: true,
      alias: 'agent-id',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const handoffPort = requireHandoffPort(services);

      const featureName = resolveFeature(services, args.feature);

      const handoffs = await handoffPort.receiveHandoffs(featureName ?? undefined, args.agentId);

      output({ handoffs }, () => {
        if (handoffs.length === 0) return 'No pending handoffs.';
        return handoffs
          .map((h) => `- ${h.beadId}: ${h.beadState.title} [${h.beadState.status}]`)
          .join('\n');
      });
    } catch (err) {
      handleCommandError('handoff-receive', err);
    }
  },
});
