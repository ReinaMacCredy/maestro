/**
 * maestro graph-insights -- show dependency graph metrics.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output, renderTable } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';
import { requireGraphPort } from '../../../../infra/utils/resolve.ts';

export default defineCommand({
  meta: { name: 'graph-insights', description: 'Show dependency graph metrics\n\nExamples:\n  maestro graph-insights\n  maestro graph-insights --json' },
  args: {},
  async run() {
    try {
      const services = getServices();
      const graphPort = requireGraphPort(services);

      const insights = await graphPort.getInsights();

      output(insights, (data) => {
        const lines: string[] = [
          `Nodes: ${data.nodeCount}  Edges: ${data.edgeCount}`,
          '',
        ];

        if (data.bottlenecks.length > 0) {
          lines.push('Bottlenecks:');
          lines.push(renderTable(
            ['ID', 'Title', 'Score'],
            data.bottlenecks.map((b) => [b.id, b.title, String(b.score)]),
          ));
          lines.push('');
        }

        if (data.criticalPath.length > 0) {
          lines.push('Critical Path:');
          lines.push(renderTable(
            ['ID', 'Title'],
            data.criticalPath.map((c) => [c.id, c.title]),
          ));
          lines.push('');
        }

        lines.push(`Velocity: ${data.velocity.closedLast7Days} closed (7d), ${data.velocity.closedLast30Days} closed (30d)`);
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('graph-insights', err);
    }
  },
});
