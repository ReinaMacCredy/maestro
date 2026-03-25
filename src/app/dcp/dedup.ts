/**
 * DCP deduplication -- detect near-duplicate memories via keyword overlap.
 * Used by consolidation pipeline and DCP metrics.
 */

import type { MemoryFileWithMeta } from '../../core/types.ts';
import { extractKeywords, computeSetOverlap } from './relevance.ts';

export interface DuplicatePair {
  a: string;
  b: string;
  overlap: number;
}

const DEDUP_THRESHOLD = 0.8;

/**
 * Find near-duplicate memory pairs (80%+ keyword overlap).
 * Returns pairs sorted by overlap descending.
 */
export function findDuplicates(
  memories: MemoryFileWithMeta[],
  threshold: number = DEDUP_THRESHOLD,
): DuplicatePair[] {
  if (memories.length < 2) return [];

  // Pre-compute keyword sets
  const keywordSets = memories.map(m => ({
    name: m.name,
    keywords: extractKeywords(m.bodyContent.slice(0, 1000) + ' ' + m.name),
  }));

  const pairs: DuplicatePair[] = [];

  for (let i = 0; i < keywordSets.length; i++) {
    for (let j = i + 1; j < keywordSets.length; j++) {
      const a = keywordSets[i];
      const b = keywordSets[j];
      const overlap = computeSetOverlap(a.keywords, b.keywords, 'simpson');
      if (overlap >= threshold) {
        pairs.push({ a: a.name, b: b.name, overlap });
      }
    }
  }

  return pairs.sort((x, y) => y.overlap - x.overlap);
}

