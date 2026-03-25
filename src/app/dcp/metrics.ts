/**
 * DCP metrics collection -- observability for context pruning.
 * Collects allocation stats, component breakdowns, and duplicate counts.
 */

import type { PruneContextResult } from './prune-context.ts';
import type { MemoryFileWithMeta } from '../../core/types.ts';
import { COMPONENT_REGISTRY } from './components.ts';
import { findDuplicates } from './dedup.ts';

export interface DcpMetrics {
  totalTokens: number;
  componentBreakdown: Record<string, number>;
  memoriesIncluded: number;
  memoriesDropped: number;
  memoriesTotal: number;
  duplicatesDetected: number;
  protectedComponents: string[];
  componentsIncluded: string[];
  componentsDropped: string[];
}

/**
 * Collect comprehensive DCP metrics from a prune result and memory list.
 */
export function collectMetrics(
  pruneResult: PruneContextResult,
  memories: MemoryFileWithMeta[],
): DcpMetrics {
  const duplicates = findDuplicates(memories);
  const { metrics } = pruneResult;

  return {
    totalTokens: metrics.totalTokens,
    componentBreakdown: { ...metrics.sections },
    memoriesIncluded: metrics.memoriesIncluded,
    memoriesDropped: metrics.memoriesDropped,
    memoriesTotal: metrics.memoriesTotal,
    duplicatesDetected: duplicates.length,
    protectedComponents: COMPONENT_REGISTRY.filter(c => c.protected).map(c => c.name),
    componentsIncluded: metrics.componentsIncluded ?? [],
    componentsDropped: metrics.componentsDropped ?? [],
  };
}

/**
 * Format metrics as a human-readable summary string.
 */
export function formatMetricsSummary(metrics: DcpMetrics): string {
  const lines: string[] = [
    `DCP Metrics:`,
    `  Total tokens: ${metrics.totalTokens}`,
    `  Memories: ${metrics.memoriesIncluded}/${metrics.memoriesTotal} included, ${metrics.memoriesDropped} dropped`,
    `  Duplicates detected: ${metrics.duplicatesDetected}`,
    `  Protected: ${metrics.protectedComponents.join(', ')}`,
  ];

  if (metrics.componentsIncluded.length > 0) {
    lines.push(`  Components included: ${metrics.componentsIncluded.join(', ')}`);
  }
  if (metrics.componentsDropped.length > 0) {
    lines.push(`  Components dropped: ${metrics.componentsDropped.join(', ')}`);
  }

  lines.push('  Budget breakdown:');
  for (const [name, tokens] of Object.entries(metrics.componentBreakdown)) {
    if (tokens > 0) lines.push(`    ${name}: ${tokens} tokens`);
  }

  return lines.join('\n');
}
