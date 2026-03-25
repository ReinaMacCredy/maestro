/**
 * Goal-based memory scoring for handoffs.
 *
 * Ranks memories by relevance to a handoff goal string.
 * Scoring: 40% keyword overlap, 25% category, 20% recency, 15% priority.
 */

import type { MemoryFileWithMeta } from '../../core/types.ts';
import { extractKeywords, scorePriority } from '../dcp/relevance.ts';

export interface ScoredMemory {
  name: string;
  score: number;
  memory: MemoryFileWithMeta;
}

export interface ScoreByGoalOpts {
  /** Maximum number of results to return. Default: no limit. */
  limit?: number;
}

/** Category relevance weights for handoff context. */
const CATEGORY_SCORES: Record<string, number> = {
  decision: 1.0,
  architecture: 0.9,
  research: 0.7,
  convention: 0.6,
  debug: 0.4,
  execution: 0.3,
};

const WEIGHTS = {
  keywordOverlap: 0.40,
  categoryMatch: 0.25,
  recency: 0.20,
  priority: 0.15,
} as const;

/**
 * Score and rank memories by relevance to a goal string.
 *
 * @param memories - Candidate memories to score.
 * @param goal - Free-text goal string describing what the handoff needs.
 * @param opts - Optional limit on results.
 * @returns Memories sorted by score descending.
 */
export function scoreByGoal(
  memories: MemoryFileWithMeta[],
  goal: string,
  opts?: ScoreByGoalOpts,
): ScoredMemory[] {
  if (memories.length === 0 || !goal) return [];

  const goalKeywords = extractKeywords(goal);
  const now = Date.now();

  const scored = memories.map(m => ({
    name: m.name,
    score: scoreOne(m, goalKeywords, now),
    memory: m,
  }));

  scored.sort((a, b) => b.score - a.score);

  if (opts?.limit && opts.limit > 0) {
    return scored.slice(0, opts.limit);
  }
  return scored;
}

function scoreOne(
  memory: MemoryFileWithMeta,
  goalKeywords: Set<string>,
  now: number,
): number {
  const kw = scoreKeywordOverlap(memory, goalKeywords);
  const cat = scoreCategoryMatch(memory.metadata.category);
  const rec = scoreRecency(memory.updatedAt, now);
  const pri = scorePriority(memory.metadata.priority);

  return (
    WEIGHTS.keywordOverlap * kw +
    WEIGHTS.categoryMatch * cat +
    WEIGHTS.recency * rec +
    WEIGHTS.priority * pri
  );
}

function scoreKeywordOverlap(
  memory: MemoryFileWithMeta,
  goalKeywords: Set<string>,
): number {
  if (goalKeywords.size === 0) return 0;

  const memText = memory.bodyContent.slice(0, 500) + ' ' + memory.name;
  const memKeywords = extractKeywords(memText);
  if (memKeywords.size === 0) return 0;

  let overlap = 0;
  for (const word of memKeywords) {
    if (goalKeywords.has(word)) overlap++;
  }

  // Normalize by the smaller set to be generous
  return overlap / Math.min(goalKeywords.size, memKeywords.size);
}

function scoreCategoryMatch(category: string | undefined): number {
  if (!category) return 0;
  return CATEGORY_SCORES[category] ?? 0;
}

function scoreRecency(updatedAt: string, now: number): number {
  const mtime = new Date(updatedAt).getTime();
  const ageMs = now - mtime;
  const dayMs = 86400000;

  // Linear decay over 30 days
  return Math.max(0, 1 - ageMs / (30 * dayMs));
}

