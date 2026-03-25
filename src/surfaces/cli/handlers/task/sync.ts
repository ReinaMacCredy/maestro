/**
 * maestro task-sync -- sync tasks from approved plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { syncPlan } from '../../../../tasks/sync-plan.ts';
import { translatePlan } from '../../../../tasks/translate-plan.ts';
import { output } from '../../../../core/output.ts';
import { handleCommandError } from '../../../../core/errors.ts';

export default defineCommand({
  meta: { name: 'task-sync', description: 'Sync tasks from approved plan' },
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
      const result = services.taskBackend === 'br'
        ? await translatePlan(services, args.feature)
        : await syncPlan(services, args.feature);

      output(result, (r) => {
        const lines = [`[ok] tasks synced for '${args.feature}'`];
        if (r.created.length > 0) lines.push(`  created: ${r.created.join(', ')}`);
        if (r.removed.length > 0) lines.push(`  removed: ${r.removed.join(', ')}`);
        if (r.kept.length > 0) lines.push(`  kept: ${r.kept.join(', ')}`);
        if (r.manual.length > 0) lines.push(`  manual (untouched): ${r.manual.join(', ')}`);
        if (r.warnings && r.warnings.length > 0) {
          for (const w of r.warnings) {
            lines.push(`  [!] ${w}`);
          }
        }
        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('task-sync', err);
    }
  },
});
