/**
 * maestro history -- show feature completion history with stats.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { history, type HistoryResult } from '../../../app/workflow/history.ts';
import { output, renderTable } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';
import type { FeatureStatusType } from '../../../domain/types.ts';

function formatDuration(days?: number): string {
  if (days === undefined) return '--';
  if (days < 1) return '<1d';
  return `${days}d`;
}

function formatHistory(result: HistoryResult): string {
  const lines: string[] = [];

  if (result.features.length === 0) {
    return 'No features found.';
  }

  const headers = ['Name', 'Status', 'Tasks', 'Created', 'Duration'];
  const rows = result.features.map((f: HistoryResult['features'][number]) => [
    f.name,
    f.status,
    `${f.taskStats.done}/${f.taskStats.total}`,
    f.createdAt.slice(0, 10),
    formatDuration(f.durationDays),
  ]);
  lines.push(renderTable(headers, rows));

  if (result.total > result.features.length) {
    lines.push('');
    lines.push(`Showing ${result.features.length} of ${result.total} features. Use --limit to see more.`);
  }

  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'history', description: 'Show feature completion history with stats' },
  args: {
    limit: {
      type: 'string',
      description: 'Max features to show (default: 10)',
    },
    status: {
      type: 'string',
      description: 'Filter by status (planning, approved, executing, completed)',
    },
  },
  async run({ args }) {
    try {
      const services = getServices();
      const result = await history(services, {
        limit: args.limit ? parseInt(args.limit, 10) : undefined,
        status: args.status as FeatureStatusType | undefined,
      });
      output(result, formatHistory);
    } catch (err) {
      handleCommandError('history', err);
    }
  },
});
