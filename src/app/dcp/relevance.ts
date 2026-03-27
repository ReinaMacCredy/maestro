/**
 * DCP relevance scoring -- scores memories against task context.
 * Deterministic, no LLM calls. All factors normalized to 0.0-1.0.
 */

import type { MemoryFileWithMeta, TaskInfo } from '../../domain/types.ts';
import { MEMORY_PREVIEW_CHARS } from '../../domain/constants.ts';
import { extractSourceTask, scoreDependencyProximity } from '../tasks/graph/proximity.ts';
import { extractKeywords, computeSetOverlap, scorePriority, STOPWORDS, TAG_WEIGHT, KEYWORD_WEIGHT } from '../../domain/text-utils.ts';

// Re-export text utilities so existing consumers don't break.
export { extractKeywords, computeSetOverlap, scorePriority, STOPWORDS, TAG_WEIGHT, KEYWORD_WEIGHT } from '../../domain/text-utils.ts';

const WEIGHTS = {
  tagOverlap: 0.30,
  categoryMatch: 0.20,
  priority: 0.15,
  recency: 0.10,
  keywordOverlap: 0.25,
} as const;

/**
 * Word-boundary tag matching.
 * Tag "auth" matches word "auth" in context, NOT substring "auth" inside "coauthored".
 * Compiled regexes are cached -- tag cardinality is practically bounded (~250 unique tags).
 */
const tagRegexCache = new Map<string, RegExp>();

function matchesTag(tag: string, context: string): boolean {
  let re = tagRegexCache.get(tag);
  if (!re) {
    const escaped = tag.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    re = new RegExp(`\\b${escaped}\\b`, 'i');
    tagRegexCache.set(tag, re);
  }
  return re.test(context);
}

function scoreTagOverlap(
  memoryTags: string[],
  tagContext: string,
): number {
  if (memoryTags.length === 0) return 0;
  const matching = memoryTags.filter(tag => matchesTag(tag, tagContext));
  return matching.length / memoryTags.length;
}

function scoreCategoryMatch(
  category: string | undefined,
  task: TaskInfo,
): number {
  if (!category) return 0;
  if (category === 'architecture' || category === 'decision') return 1.0;
  if (category === 'convention') return 0.75;
  if (category === 'debug') return task.status === 'blocked' ? 1.0 : 0.0;
  if (category === 'research') return 0.5;
  if (category === 'execution') return 0.30;
  return 0;
}

function scoreRecency(
  memoryUpdatedAt: string,
  featureCreatedAt: string,
): number {
  const now = Date.now();
  const mtime = new Date(memoryUpdatedAt).getTime();
  const created = new Date(featureCreatedAt).getTime();
  const featureAge = Math.max(now - created, 3600000); // floor 1 hour
  const memoryAge = now - mtime;
  return 1 - Math.max(0, Math.min(1, memoryAge / featureAge));
}

function scoreKeywordOverlap(
  bodyContent: string,
  fileName: string,
  taskKeywords: Set<string>,
): number {
  const memoryText = bodyContent.slice(0, MEMORY_PREVIEW_CHARS) + ' ' + fileName;
  const memoryWords = extractKeywords(memoryText);
  if (memoryWords.size === 0) return 0;

  let intersection = 0;
  for (const word of memoryWords) {
    if (taskKeywords.has(word)) intersection++;
  }

  return intersection / memoryWords.size;
}

/** Pre-computed task context to avoid redundant work when scoring multiple memories. */
export interface TaskContext {
  tagContext: string;
  taskKeywords: Set<string>;
}

/** Pre-computed proximity context for dependency-based scoring. */
export interface ProximityContext {
  downstreamMap: Map<string, string[]>;
  taskFolders: Set<string>;
}

/** Build task context once, pass to scoreRelevance for each memory. */
export function buildTaskContext(task: TaskInfo, planSection: string | null): TaskContext {
  const tagContext = [task.name, task.id, planSection ?? ''].join(' ');
  const taskKeywords = extractKeywords(tagContext);
  return { tagContext, taskKeywords };
}

/**
 * Score a memory's relevance to a task. Returns 0.0-1.0.
 * Pass pre-built TaskContext to avoid redundant computation across multiple memories.
 */
export function scoreRelevance(
  memory: MemoryFileWithMeta,
  task: TaskInfo,
  planSection: string | null,
  featureCreatedAt?: string,
  precomputed?: TaskContext,
  proximityCtx?: ProximityContext,
): number {
  const ctx = precomputed ?? buildTaskContext(task, planSection);
  const tags = memory.metadata.tags ?? [];

  const tagScore = scoreTagOverlap(tags, ctx.tagContext);
  const categoryScore = scoreCategoryMatch(memory.metadata.category, task);
  const priorityScore = scorePriority(memory.metadata.priority);
  const recencyScore = featureCreatedAt
    ? scoreRecency(memory.updatedAt, featureCreatedAt)
    : 0.5;
  const keywordScore = scoreKeywordOverlap(
    memory.bodyContent, memory.name, ctx.taskKeywords,
  );

  let score =
    WEIGHTS.tagOverlap * tagScore +
    WEIGHTS.categoryMatch * categoryScore +
    WEIGHTS.priority * priorityScore +
    WEIGHTS.recency * recencyScore +
    WEIGHTS.keywordOverlap * keywordScore;

  if (proximityCtx) {
    const source = extractSourceTask(memory.name);
    if (source && proximityCtx.taskFolders.has(source)) {
      score = Math.min(1.0, score + scoreDependencyProximity(source, task.id, proximityCtx.downstreamMap));
    }
  }

  return score;
}
