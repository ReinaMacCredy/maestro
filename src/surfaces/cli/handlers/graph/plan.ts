/**
 * maestro graph-plan -- show parallel execution tracks.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';
import { requireGraphPort } from '../../../../core/resolve.ts';

export default defineCommand({
  meta: { name: 'graph-plan', description: 'Show parallel execution tracks' },
  args: {
    agents: {
      type: 'string',
      description: 'Number of parallel agents (default: 1)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const graphPort = requireGraphPort(services);

      const plan = await graphPort.getExecutionPlan(parseInt(args.agents || '1', 10));

      output(plan, (data) => {
        const lines = [`Parallelism: ${data.parallelism}`, ''];
        for (const track of data.tracks) {
          lines.push(`Track: ${track.name}`);
          for (const bead of track.beads) {
            lines.push(`  ${bead.order}. ${bead.id} -- ${bead.title}`);
          }
          lines.push('');
        }
        return lines.join('\n').trimEnd();
      });
    } catch (err) {
      handleCommandError('graph-plan', err);
    }
  },
});
