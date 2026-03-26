/**
 * maestro status -- composite feature status query.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { checkStatus, type StatusResult } from '../../../app/workflow/status.ts';
import { output, renderStatusLine } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../infra/utils/resolve.ts';
import { truncateList, formatTruncation } from '../../../infra/utils/truncation.ts';

function formatStatus(result: StatusResult): string {
  const lines: string[] = [];

  lines.push('# maestro status');
  lines.push('');
  lines.push(renderStatusLine('feature', `${result.feature.name} [${result.feature.status}]`));

  const planLabel = result.plan.exists
    ? (result.plan.approved ? 'approved' : 'draft')
    : 'none';
  const commentSuffix = result.plan.commentCount > 0 ? ` (${result.plan.commentCount} comments)` : '';
  lines.push(renderStatusLine('plan', `${planLabel}${commentSuffix}`));

  const taskSummary = `${result.tasks.done}/${result.tasks.total} done, ` +
    `${result.tasks.inProgress} claimed, ${result.tasks.pending} pending`;
  lines.push(renderStatusLine('tasks', taskSummary));

  const blockedSet = new Set(result.blocked);
  const taskLines = result.tasks.items.map(t => {
    const status = `[${t.status}]`.padEnd(12);
    const suffix = blockedSet.has(t.id) ? ' (blocked)' : '';
    return `  ${status} ${t.id}${suffix}`;
  });
  const { items: visibleTasks, truncated } = truncateList(taskLines, 20);
  lines.push(...visibleTasks);
  if (truncated > 0) {
    lines.push(`  ${formatTruncation(truncated, 'tasks')}`);
  }

  if (result.context.count > 0) {
    lines.push(renderStatusLine('context', `${result.context.count} files, ~${result.context.totalBytes} bytes`));
  }

  if (result.dcp) {
    const dcpLabel = result.dcp.enabled
      ? `on (budget: ${result.dcp.memoryBudgetTokens}T, current: ${result.context.totalBytes}B, ${result.context.count} files)`
      : 'off';
    lines.push(renderStatusLine('dcp', dcpLabel));
  }

  lines.push(renderStatusLine('next', result.nextAction));

  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'status', description: 'Show feature status overview\n\nExamples:\n  maestro status\n  maestro status --feature my-feature --json' },
  args: {
    feature: {
      type: 'string',
      description: 'Feature name (uses active feature if omitted)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();

      const featureName = requireFeature(services, args.feature, [
        FEATURE_HINT,
      ]);

      const result = await checkStatus(services, featureName);
      output(result, formatStatus);
    } catch (err) {
      handleCommandError('status', err);
    }
  },
});
