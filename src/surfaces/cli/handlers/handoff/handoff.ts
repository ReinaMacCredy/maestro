/**
 * maestro handoff-ack -- acknowledge a handoff.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireHandoffPort } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'handoff-ack', description: 'Acknowledge a handoff' },
  args: {
    threadId: {
      type: 'string',
      description: 'Agent Mail thread ID to acknowledge',
      required: true,
      alias: 'thread-id',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const handoffPort = requireHandoffPort(services);

      await handoffPort.acknowledgeHandoff(args.threadId);

      output({ threadId: args.threadId }, () =>
        `[ok] acknowledged thread '${args.threadId}'`,
      );
    } catch (err) {
      handleCommandError('handoff-ack', err);
    }
  },
});
