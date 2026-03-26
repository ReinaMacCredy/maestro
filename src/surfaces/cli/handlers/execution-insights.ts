/**
 * maestro execution-insights -- query the execution knowledge graph.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { executionInsights, type ExecutionInsightsResult } from '../../../app/workflow/insights.ts';
import { output } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';
import { requireFeature, FEATURE_HINT } from '../../../infra/utils/resolve.ts';

function formatInsights(result: ExecutionInsightsResult): string {
  const lines: string[] = [];

  lines.push(`# Execution Insights: ${result.feature}`);
  lines.push('');
  lines.push(`Coverage: ${result.coverage.withExecMemory}/${result.coverage.totalTasks} tasks (${result.coverage.percent}%)`);
  lines.push('');

  if (result.insights.length === 0) {
    lines.push('No execution memories found.');
    return lines.join('\n');
  }

  lines.push('## Insights');
  for (const insight of result.insights) {
    const verified = insight.verificationPassed ? '[ok]' : '[!]';
    lines.push(`  ${verified} ${insight.sourceTask}: ${insight.summary}`);
    if (insight.filesChanged > 0) {
      lines.push(`      files: ${insight.filesChanged}`);
    }
    if (insight.downstreamTasks.length > 0) {
      lines.push(`      downstream: ${insight.downstreamTasks.join(', ')}`);
    }
  }

  if (result.knowledgeFlow.length > 0) {
    lines.push('');
    lines.push('## Knowledge Flow');
    for (const edge of result.knowledgeFlow) {
      lines.push(`  ${edge.from} --> ${edge.to} (proximity: ${edge.proximity.toFixed(2)})`);
    }
  }

  if (result.doctrineEffectiveness && result.doctrineEffectiveness.length > 0) {
    lines.push('');
    lines.push('## Doctrine Effectiveness');
    for (const d of result.doctrineEffectiveness) {
      const staleTag = d.stale ? ' [stale]' : '';
      lines.push(`  ${d.name}: injected ${d.injectionCount}x, success ${(d.successRate * 100).toFixed(0)}%, overrides ${d.overrideCount}${staleTag}`);
    }
  }

  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'execution-insights', description: 'Query execution knowledge graph\n\nExamples:\n  maestro execution-insights --feature my-feat\n  maestro execution-insights --feature my-feat --json' },
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

      const result = await executionInsights(
        featureName,
        services.taskPort,
        services.memoryAdapter,
        services.doctrinePort,
        services.settingsPort.get().doctrine,
      );
      output(result, formatInsights);
    } catch (err) {
      handleCommandError('execution-insights', err);
    }
  },
});
