/**
 * maestro graph-next -- show recommended next bead.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireGraphPort } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'graph-next', description: 'Show recommended next bead\n\nExamples:\n  maestro graph-next\n  maestro graph-next --json' },
  args: {},
  async run() {
    try {
      const services = getServices();
      const graphPort = requireGraphPort(services);

      const recommendation = await graphPort.getNextRecommendation();

      if (!recommendation) {
        output({ message: 'No recommendations available' }, () => 'No recommendations available.');
        return;
      }

      output(recommendation, (rec) => {
        const lines = [
          `Recommended: ${rec.id}`,
          `  Title: ${rec.title}`,
          `  Score: ${rec.score}`,
          `  Unblocks: ${rec.unblocks}`,
        ];
        if (rec.reasons.length > 0) {
          lines.push('  Reasons:');
          rec.reasons.forEach((r) => lines.push(`    - ${r}`));
        }
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('graph-next', err);
    }
  },
});
