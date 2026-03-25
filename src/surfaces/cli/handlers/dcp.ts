/**
 * maestro dcp-preview -- preview DCP memory selection for a task.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { output, renderTable } from '../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../infra/utils/resolve.ts';
import { pruneContext } from '../../../app/dcp/prune-context.ts';
import { resolveDcpConfig } from '../../../app/dcp/config.ts';
import { WORKER_RULES } from '../../../app/tasks/worker-rules.ts';

export default defineCommand({
  meta: { name: 'dcp-preview', description: 'Preview DCP memory selection for a task' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (uses active feature if omitted)',
    },
    task: {
      type: 'string',
      description: 'Task folder ID',
      required: true,
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const feature = requireFeature(services, args.feature, [
        'Specify --feature <name>',
      ]);

      const task = await services.taskPort.get(feature, args.task);
      if (!task) {
        throw new MaestroError(`Task '${args.task}' not found in feature '${feature}'`);
      }

      const spec = await services.taskPort.readSpec(feature, args.task) ?? '(no spec)';
      const memories = services.memoryAdapter.listWithMeta(feature);
      const resolvedDcp = resolveDcpConfig(services.settingsPort.get().dcp);

      const featureInfo = services.featureAdapter.get(feature);
      const featureCreatedAt = featureInfo?.createdAt;

      // Load all tasks for dependency-proximity scoring
      const allTasks = await services.taskPort.list(feature, { includeAll: true });
      const taskDeps = allTasks.map(t => ({
        id: t.id, folder: t.folder, status: t.status, dependsOn: t.dependsOn,
      }));

      const { metrics } = pruneContext({
        featureName: feature,
        taskFolder: args.task,
        task,
        spec,
        memories,
        richContext: '',
        graphContext: '',
        workerRules: WORKER_RULES,
        dcpConfig: resolvedDcp,
        featureCreatedAt,
        allTasks: taskDeps,
      });

      output(metrics, (m) => {
        const lines: string[] = [];
        lines.push(`# DCP Preview: ${args.task}`);
        lines.push(`  feature:  ${feature}`);
        lines.push(`  enabled:  ${resolvedDcp.enabled}`);
        lines.push(`  budget:   ${resolvedDcp.memoryBudgetTokens} tokens`);
        lines.push(`  memories: ${m.memoriesIncluded}/${m.memoriesTotal} included, ${m.memoriesDropped} dropped`);
        lines.push('');

        if (m.scores.length > 0) {
          const rows = m.scores.map(s => [
            s.name,
            (Math.round(s.score * 1000) / 1000).toString(),
            s.included ? '[ok]' : '[x]',
          ]);
          lines.push(renderTable(['Memory', 'Score', 'Included'], rows));
        } else {
          lines.push('  No memories found.');
        }

        return lines.join('\n');
      });
    } catch (err) {
      handleCommandError('dcp-preview', err);
    }
  },
});
