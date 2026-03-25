/**
 * Suggest doctrine candidates from cross-feature execution patterns.
 * All heuristic, no LLM. Requires minSampleSize features with matching pattern.
 */

import type { FeaturePort } from '../../features/port.ts';
import type { MemoryPort } from '../../memory/port.ts';
import type { DoctrineItem, DoctrineConditions } from '../../doctrine/port.ts';
import { isExecutionMemory } from '../memory/execution/writer.ts';
import { parseExecMemory, type ParsedExecMemory, groupByTagCluster, listRecentFeatures } from '../memory/execution/parser.ts';
import { extractKeywords } from '../dcp/relevance.ts';
import { titleToSlug } from '../../core/slug.ts';
import { resolveDoctrineConfig } from './config.ts';
import type { DoctrineSettings } from '../../core/settings.ts';

export interface DoctrineSuggestion {
  name: string;
  rule: string;
  rationale: string;
  conditions: DoctrineConditions;
  tags: string[];
  source: { features: string[]; memories: string[] };
  confidence: 'low' | 'medium' | 'high';
  category: 'failure-prevention' | 'testing' | 'complexity-warning' | 'positive-pattern';
}

export interface SuggestDoctrineResult {
  suggestions: DoctrineSuggestion[];
  analysisStats: {
    execMemoriesAnalyzed: number;
    crossFeatureMatches: number;
  };
}

interface TaggedMemory {
  featureName: string;
  memoryName: string;
  parsed: ParsedExecMemory;
}

function slugify(tags: string[], category: string): string {
  return titleToSlug(`${category}-${tags.slice(0, 3).join('-')}`);
}

/** Pre-computed keyword sets for existing doctrine items (avoids re-extracting per candidate). */
interface DoctrineWithKeywords {
  item: DoctrineItem;
  tagSet: Set<string>;
  keywords: Set<string>;
}

function precomputeExisting(existing: DoctrineItem[]): DoctrineWithKeywords[] {
  return existing.map(item => ({
    item,
    tagSet: new Set(item.tags.map(t => t.toLowerCase())),
    keywords: extractKeywords(item.rule),
  }));
}

function isDuplicate(suggestion: DoctrineSuggestion, existing: DoctrineWithKeywords[]): boolean {
  const suggTags = suggestion.tags.map(t => t.toLowerCase());
  const suggKeywords = extractKeywords(suggestion.rule);

  for (const doc of existing) {
    const tagOverlap = suggTags.filter(t => doc.tagSet.has(t)).length / Math.max(suggTags.length, 1);
    if (tagOverlap <= 0.7) continue;

    let keywordMatches = 0;
    for (const kw of suggKeywords) {
      if (doc.keywords.has(kw)) keywordMatches++;
    }
    const keywordOverlap = suggKeywords.size > 0 ? keywordMatches / suggKeywords.size : 0;
    if (keywordOverlap > 0.5) return true;
  }
  return false;
}

export function suggestDoctrine(
  featureAdapter: FeaturePort,
  memoryAdapter: MemoryPort,
  existingDoctrine: DoctrineItem[],
  doctrineConfig?: DoctrineSettings,
): SuggestDoctrineResult {
  const cfg = resolveDoctrineConfig(doctrineConfig);
  const { minSampleSize, maxSuggestionsPerFeature, crossFeatureScanLimit } = cfg;

  const scanned = listRecentFeatures(featureAdapter, crossFeatureScanLimit);

  const allMemories: TaggedMemory[] = [];
  for (const { name } of scanned) {
    try {
      const memories = memoryAdapter.listWithMeta(name);
      for (const m of memories) {
        if (!isExecutionMemory(m.name)) continue;
        allMemories.push({ featureName: name, memoryName: m.name, parsed: parseExecMemory(m.content) });
      }
    } catch {
      continue;
    }
  }

  // Group by tag clusters and analyze patterns
  const clusters = groupByTagCluster(allMemories);
  const suggestions: DoctrineSuggestion[] = [];
  const existingPrecomputed = precomputeExisting(existingDoctrine);

  for (const [clusterKey, memories] of clusters) {
    const uniqueFeatures = [...new Set(memories.map(m => m.featureName))];
    if (uniqueFeatures.length < minSampleSize) continue;

    const tags = clusterKey.split('+');

    // Failure prevention: high revisions
    const avgRevisions = memories.reduce((sum, m) => sum + m.parsed.revisionCount, 0) / memories.length;
    if (avgRevisions > 0.5) {
      const suggestion: DoctrineSuggestion = {
        name: slugify(tags, 'prevent'),
        rule: `Tasks involving [${tags.join(', ')}] frequently require revisions (${avgRevisions.toFixed(1)}x avg). Plan extra verification and test coverage.`,
        rationale: `Observed ${avgRevisions.toFixed(1)}x average revisions across ${uniqueFeatures.length} features with this tag pattern.`,
        conditions: { tags },
        tags,
        source: { features: uniqueFeatures, memories: memories.map(m => m.memoryName) },
        confidence: 'high',
        category: 'failure-prevention',
      };
      if (!isDuplicate(suggestion, existingPrecomputed)) suggestions.push(suggestion);
    }

    // Testing pitfall: verification failures
    const failRate = memories.filter(m => !m.parsed.verificationPassed).length / memories.length;
    if (failRate > 0.2 && avgRevisions <= 0.5) {
      const suggestion: DoctrineSuggestion = {
        name: slugify(tags, 'testing'),
        rule: `Tasks involving [${tags.join(', ')}] have a ${(failRate * 100).toFixed(0)}% verification failure rate. Add explicit verification steps.`,
        rationale: `Observed ${(failRate * 100).toFixed(0)}% verification failure rate across ${uniqueFeatures.length} features.`,
        conditions: { tags },
        tags,
        source: { features: uniqueFeatures, memories: memories.map(m => m.memoryName) },
        confidence: 'medium',
        category: 'testing',
      };
      if (!isDuplicate(suggestion, existingPrecomputed)) suggestions.push(suggestion);
    }

    // Positive pattern: consistent success
    const allSucceeded = memories.every(m => m.parsed.revisionCount === 0 && m.parsed.verificationPassed);
    if (allSucceeded && memories.length >= minSampleSize) {
      const suggestion: DoctrineSuggestion = {
        name: slugify(tags, 'positive'),
        rule: `Tasks involving [${tags.join(', ')}] consistently succeed first try. Maintain current patterns.`,
        rationale: `${memories.length} tasks across ${uniqueFeatures.length} features completed without revision.`,
        conditions: { tags },
        tags,
        source: { features: uniqueFeatures, memories: memories.map(m => m.memoryName) },
        confidence: 'medium',
        category: 'positive-pattern',
      };
      if (!isDuplicate(suggestion, existingPrecomputed)) suggestions.push(suggestion);
    }
  }

  // Sort by confidence (high first), cap at maxSuggestionsPerFeature
  const confidenceOrder = { high: 0, medium: 1, low: 2 };
  suggestions.sort((a, b) => confidenceOrder[a.confidence] - confidenceOrder[b.confidence]);

  return {
    suggestions: suggestions.slice(0, maxSuggestionsPerFeature),
    analysisStats: {
      execMemoriesAnalyzed: allMemories.length,
      crossFeatureMatches: [...clusters.values()].filter(c => {
        const feats = new Set(c.map(m => m.featureName));
        return feats.size >= minSampleSize;
      }).length,
    },
  };
}
