/**
 * Query cross-feature execution history for plan-time awareness.
 * Scores exec memories against plan content by tag/keyword overlap,
 * groups by tag clusters, and surfaces failure patterns as pitfalls.
 *
 * Deterministic, no LLM calls.
 */

import type { FeaturePort } from '../../features/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import type { MemoryFileWithMeta } from '../../core/types.ts';
import { isExecutionMemory } from '../memory/execution/writer.ts';
import { parseExecMemory, type ParsedExecMemory, groupByTagCluster, listRecentFeatures } from '../memory/execution/parser.ts';
import { extractKeywords, TAG_WEIGHT, KEYWORD_WEIGHT } from './relevance.ts';
import { formatDurationMinutes, parseDurationMinutes } from '../../core/time-utils.ts';

export interface HistoricalPitfall {
  pattern: string;
  metric: string;
  sourceFeatures: string[];
  sourceTasks: string[];
  severity: 'low' | 'medium' | 'high';
}

export interface HistoricalContextResult {
  pitfalls: HistoricalPitfall[];
  totalExecMemoriesScanned: number;
  featuresScanned: number;
}

interface ScoredExecMemory {
  featureName: string;
  memoryName: string;
  parsed: ParsedExecMemory;
  score: number;
}

const OVERLAP_THRESHOLD = 0.3;
const DEFAULT_SCAN_LIMIT = 20;

/**
 * Score a memory's tag+keyword overlap with plan keywords.
 * Returns 0.0-1.0 (higher = more relevant).
 */
function scoreOverlap(memTags: string[], planKeywords: Set<string>): number {
  if (planKeywords.size === 0) return 0;

  let matches = 0;
  for (const tag of memTags) {
    if (planKeywords.has(tag.toLowerCase())) matches++;
  }
  const tagScore = memTags.length > 0 ? matches / memTags.length : 0;

  const keywordScore = planKeywords.size > 0 ? matches / planKeywords.size : 0;

  return tagScore * TAG_WEIGHT + keywordScore * KEYWORD_WEIGHT;
}

export function queryHistoricalContext(
  planContent: string,
  featureAdapter: FeaturePort,
  memoryAdapter: MemoryPort,
  opts?: { scanLimit?: number },
): HistoricalContextResult {
  const scanLimit = opts?.scanLimit ?? DEFAULT_SCAN_LIMIT;
  const planKeywords = extractKeywords(planContent);

  if (planKeywords.size === 0) {
    return { pitfalls: [], totalExecMemoriesScanned: 0, featuresScanned: 0 };
  }

  const scannedFeatures = listRecentFeatures(featureAdapter, scanLimit);

  // Collect and score exec memories across features
  const scoredMemories: ScoredExecMemory[] = [];
  let totalScanned = 0;

  for (const { name: featureName } of scannedFeatures) {
    let memories: MemoryFileWithMeta[];
    try {
      memories = memoryAdapter.listWithMeta(featureName);
    } catch {
      continue; // Feature may have been deleted or corrupted
    }

    const execMemories = memories.filter(m => isExecutionMemory(m.name));
    totalScanned += execMemories.length;

    for (const mem of execMemories) {
      const parsed = parseExecMemory(mem.content);
      const score = scoreOverlap(parsed.tags, planKeywords);

      if (score >= OVERLAP_THRESHOLD) {
        scoredMemories.push({
          featureName,
          memoryName: mem.name,
          parsed,
          score,
        });
      }
    }
  }

  // Group by tag clusters and compute aggregates
  const clusters = groupByTagCluster(scoredMemories);
  const pitfalls: HistoricalPitfall[] = [];

  for (const [clusterKey, memories] of clusters) {
    const uniqueFeatures = [...new Set(memories.map(m => m.featureName))];
    if (uniqueFeatures.length < 2) continue; // Need cross-feature signal

    const revisions = memories.map(m => m.parsed.revisionCount);
    const avgRevisions = revisions.reduce((a, b) => a + b, 0) / revisions.length;

    const verificationFailRate = memories.filter(m => !m.parsed.verificationPassed).length / memories.length;

    const durations = memories
      .map(m => m.parsed.duration)
      .filter((d): d is string => d !== undefined)
      .map(parseDurationMinutes)
      .filter((d): d is number => d !== undefined);
    const avgDuration = durations.length > 0
      ? durations.reduce((a, b) => a + b, 0) / durations.length
      : undefined;

    const tags = clusterKey.split('+');
    const sourceTasks = memories.map(m => m.memoryName);

    if (avgRevisions > 0.5) {
      pitfalls.push({
        pattern: `tasks involving [${tags.join(', ')}]`,
        metric: `${avgRevisions.toFixed(1)}x average revisions across ${uniqueFeatures.length} features`,
        sourceFeatures: uniqueFeatures,
        sourceTasks,
        severity: 'high',
      });
    } else if (verificationFailRate > 0.2) {
      pitfalls.push({
        pattern: `tasks involving [${tags.join(', ')}]`,
        metric: `${(verificationFailRate * 100).toFixed(0)}% verification failure rate across ${uniqueFeatures.length} features`,
        sourceFeatures: uniqueFeatures,
        sourceTasks,
        severity: 'medium',
      });
    } else if (avgDuration !== undefined && avgDuration > 120) {
      pitfalls.push({
        pattern: `tasks involving [${tags.join(', ')}]`,
        metric: `${formatDurationMinutes(avgDuration)} average duration across ${uniqueFeatures.length} features`,
        sourceFeatures: uniqueFeatures,
        sourceTasks,
        severity: 'low',
      });
    }
  }

  // Sort by severity (high first)
  const severityOrder = { high: 0, medium: 1, low: 2 };
  pitfalls.sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity]);

  return {
    pitfalls,
    totalExecMemoriesScanned: totalScanned,
    featuresScanned: scannedFeatures.length,
  };
}

