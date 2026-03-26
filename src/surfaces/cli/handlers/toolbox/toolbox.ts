/**
 * maestro toolbox-list -- list all registered tools with status.
 * This is the default/parent toolbox command.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'toolbox-list', description: 'List all registered tools with status and transport\n\nExamples:\n  maestro toolbox-list\n  maestro toolbox-list --json' },
  args: {},
  async run() {
    try {
      const { toolbox, agentToolsRegistry } = getServices();
      const statuses = toolbox.getStatus();
      const agentTools = agentToolsRegistry.getAll();

      const portTools = statuses.map((s) => ({
        name: s.manifest.name,
        type: 'port' as const,
        transport: s.transport,
        status: s.settingsState === 'denied'
          ? 'denied'
          : s.installed ? 'installed' : 'missing',
        version: s.version ?? null,
        provides: s.manifest.provides,
        category: null as string | null,
        description: s.manifest.description ?? null,
      }));

      const agentEntries = agentTools.map((at) => ({
        name: at.manifest.name,
        type: 'agent' as const,
        transport: 'cli' as const,
        status: at.installed ? 'installed' : 'missing',
        version: at.version ?? null,
        provides: null as string | null,
        category: at.manifest.category,
        description: at.manifest.description ?? null,
      }));

      const data = [...portTools, ...agentEntries];

      output(data, (tools) => {
        const ports = tools.filter(t => t.type === 'port');
        const agents = tools.filter(t => t.type === 'agent');
        const lines: string[] = [];

        lines.push('[toolbox] Port tools:\n');
        for (const t of ports) {
          const status = t.status === 'installed' ? '[ok]' : t.status === 'denied' ? '[x]' : '[!]';
          const ver = t.version ? ` (${t.version})` : '';
          const port = t.provides ? ` --> ${t.provides}` : '';
          lines.push(`  ${status} ${t.name}  [${t.transport}]${ver}${port}`);
        }

        lines.push('\n[toolbox] Agent tools:\n');
        for (const t of agents) {
          const status = t.status === 'installed' ? '[ok]' : '[!]';
          const ver = t.version ? ` (${t.version})` : '';
          const cat = t.category ? ` [${t.category}]` : '';
          lines.push(`  ${status} ${t.name}${cat}${ver}`);
        }

        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('toolbox-list', err);
    }
  },
});
