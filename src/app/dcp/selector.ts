/**
 * Budget-aware memory selection for DCP.
 * Greedily fills budget with highest-scoring memories.
 * Uses token estimation (chars/4) for budget accounting.
 */

import type { MemoryFileWithMeta, TaskInfo } from '../../core/types.ts';
import type { TaskWithDeps } from '../tasks/graph/dependency.ts';
import { scoreRelevance, buildTaskContext, type ProximityContext } from './relevance.ts';
import { buildDownstreamMap } from '../tasks/graph/proximity.ts';
import { estimateTokens } from '../../core/tokens.ts';

export interface SelectedContext {
  memories: MemoryFileWithMeta[];  // ordered by score desc, within budget
  totalTokens: number;
  totalBytes: number;              // backward compat: totalTokens * 4
  includedCount: number;
  droppedCount: number;
  scores: Array<{ name: string; score: number; included: boolean }>;
}

/**
 * Select the most relevant memories within a token budget.
 *
 * - Scores each memory with scoreRelevance()
 * - Filters by relevanceThreshold (but always keeps top-1)
 * - Greedily fills budget in score order
 */
export function selectMemories(
  memories: MemoryFileWithMeta[],
  task: TaskInfo,
  planSection: string | null,
  budgetTokens: number,
  relevanceThreshold: number = 0.1,
  featureCreatedAt?: string,
  allTasks?: TaskWithDeps[],
): SelectedContext {
  if (memories.length === 0) {
    return { memories: [], totalTokens: 0, totalBytes: 0, includedCount: 0, droppedCount: 0, scores: [] };
  }

  const taskCtx = buildTaskContext(task, planSection);
  const proximityCtx: ProximityContext | undefined = allTasks
    ? {
        downstreamMap: buildDownstreamMap(allTasks),
        // Include both id and folder for backward compat (old exec-01-slug, new exec-slug)
        taskFolders: new Set(allTasks.flatMap(t => [t.id, t.folder])),
      }
    : undefined;

  if (budgetTokens <= 0) {
    const scores = memories.map(m => ({
      name: m.name,
      score: scoreRelevance(m, task, planSection, featureCreatedAt, taskCtx, proximityCtx),
      included: false,
    }));
    return { memories: [], totalTokens: 0, totalBytes: 0, includedCount: 0, droppedCount: memories.length, scores };
  }

  const scored = memories.map(m => ({
    memory: m,
    score: scoreRelevance(m, task, planSection, featureCreatedAt, taskCtx, proximityCtx),
    tokens: estimateTokens(m.bodyContent),
  }));

  // Sort by score descending
  scored.sort((a, b) => b.score - a.score);

  // Filter by threshold but always keep top-1
  const eligible = scored.filter((s, i) => i === 0 || s.score >= relevanceThreshold);

  // Greedily fill budget (skip oversized items, don't stop -- smaller items may still fit)
  const included: typeof scored = [];
  let usedTokens = 0;
  for (const entry of eligible) {
    if (usedTokens + entry.tokens <= budgetTokens) {
      included.push(entry);
      usedTokens += entry.tokens;
    }
  }

  const includedNames = new Set(included.map(i => i.memory.name));
  const scores = scored.map(s => ({
    name: s.memory.name,
    score: s.score,
    included: includedNames.has(s.memory.name),
  }));

  return {
    memories: included.map(i => i.memory),
    totalTokens: usedTokens,
    totalBytes: usedTokens * 4,
    includedCount: included.length,
    droppedCount: memories.length - included.length,
    scores,
  };
}

/**
 * Record DCP selections on the memory adapter.
 * Call after selectMemories() to track selection frequency in frontmatter.
 */
export function recordSelections(
  memoryAdapter: { recordSelection(feature: string, name: string): void },
  feature: string,
  selectedNames: string[],
): void {
  for (const name of selectedNames) {
    try { memoryAdapter.recordSelection(feature, name); } catch { /* best-effort */ }
  }
}
