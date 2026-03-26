/**
 * maestro task-sync -- sync tasks from approved plan.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { syncPlan } from '../../../../app/tasks/sync-plan.ts';
import { translatePlan } from '../../../../app/tasks/translate-plan.ts';
import { output } from '../../../../infra/utils/output.ts';
import { handleCommandError } from '../../../../domain/errors.ts';

export default defineCommand({
  meta: { name: 'task-sync', description: 'Sync tasks from approved plan\n\nExamples:\n  maestro task-sync --feature my-feat\n  maestro task-sync --feature my-feat --dry-run' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name',
      required: true,
    },
    'dry-run': {
      type: 'boolean',
      description: 'Preview sync without creating or removing tasks',
      default: false,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const dryRun = args['dry-run'];
      const result = services.taskBackend === 'br'
        ? await translatePlan(services, args.feature, { dryRun })
        : await syncPlan(services, args.feature, { dryRun });

      output(result, (r) => {
        const lines = [`[ok] tasks synced for '${args.feature}'${dryRun ? ' (dry run)' : ''}`];
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
