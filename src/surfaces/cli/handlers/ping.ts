/**
 * maestro ping -- version and integration health check.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { ping, type PingResult } from '../../../workflow/ping.ts';
import { output, renderStatusLine } from '../../../core/output.ts';
import { handleCommandError } from '../../../core/errors.ts';

function formatPing(result: PingResult): string {
  const lines: string[] = [];

  lines.push(`maestro ${result.version} [ok]`);
  lines.push('');
  lines.push(renderStatusLine('project', result.projectRoot));
  lines.push(renderStatusLine('task-backend', result.taskBackend));
  lines.push('');
  lines.push('integrations:');
  for (const [name, available] of Object.entries(result.integrations)) {
    lines.push(`  ${name}: ${available ? 'yes' : 'no'}`);
  }

  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'ping', description: 'Version and integration health check' },
  args: {},
  async run() {
    try {
      const services = getServices();
      const result = ping(services);
      output(result, formatPing);
    } catch (err) {
      handleCommandError('ping', err);
    }
  },
});
