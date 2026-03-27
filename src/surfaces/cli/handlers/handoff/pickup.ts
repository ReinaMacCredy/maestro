/**
 * maestro handoff-pickup -- discover and read a pending cross-agent handoff.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { pickupCrossAgentHandoff } from '../../../../app/handoff/crossagent.ts';

export default defineCommand({
  meta: { name: 'handoff-pickup', description: 'Discover and read a pending cross-agent handoff\n\nExamples:\n  maestro handoff-pickup --json\n  maestro handoff-pickup --feature my-feat --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (if omitted, scans for any pending handoff)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();

      const result = pickupCrossAgentHandoff(services.directory, args.feature);

      output(result, (r) => {
        let text = `[ok] handoff picked up for '${r.feature}' (${r.tasks.length} tasks)`;
        text += `\n  status: ${r.state.status}`;
        text += `\n  from: ${r.state.fromHost}`;
        return text;
      });
    } catch (err) {
      handleCommandError('handoff-pickup', err);
    }
  },
});
