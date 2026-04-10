import type { Task } from "../domain/task-types.js";
import type {
  TaskCandidate,
  CandidateSourceType,
} from "../domain/task-candidate.js";
import { extractKeywords } from "../domain/extract-keywords.js";

/**
 * A hint attached to a ready task, sourced from a past candidate. The
 * sourceType discriminator is preserved so consumers can filter or style
 * hints by origin as new candidate sources come online.
 */
export interface TaskHint {
  readonly sourceTaskId: string;
  readonly sourceType: CandidateSourceType;
  readonly title: string;
  readonly reason: string;
  readonly capturedAt: string;
  readonly matchedKeywords: readonly string[];
}

/**
 * Reverse index from a keyword to the candidates that carry it, built once
 * per `task ready` invocation so N ready tasks reuse the same lookup.
 */
export interface CandidateIndex {
  readonly byKeyword: ReadonlyMap<string, readonly TaskCandidate[]>;
  readonly size: number;
}

const DEFAULT_MAX_HINTS = 3;
const EMPTY_INDEX: CandidateIndex = {
  byKeyword: new Map<string, readonly TaskCandidate[]>(),
  size: 0,
};

/**
 * Build a keyword -> candidates index. Call once per ready batch; each
 * `matchCandidatesInIndex` call against it is cheap.
 */
export function buildCandidateIndex(
  candidates: readonly TaskCandidate[],
): CandidateIndex {
  if (candidates.length === 0) return EMPTY_INDEX;
  const byKeyword = new Map<string, TaskCandidate[]>();
  for (const candidate of candidates) {
    for (const kw of candidate.keywords) {
      let bucket = byKeyword.get(kw);
      if (bucket === undefined) {
        bucket = [];
        byKeyword.set(kw, bucket);
      }
      bucket.push(candidate);
    }
  }
  return { byKeyword, size: candidates.length };
}

/**
 * Compute hints for a single ready task against a prebuilt candidate index.
 *
 * Rules:
 *   - A candidate matches when at least one of its keywords appears in the
 *     target task's own extracted title/labels keywords.
 *   - A task never sees its own past close as a hint (self-exclusion by
 *     sourceTaskId — handles the reopened-task edge case).
 *   - Ties are broken by capturedAt DESC (newer hints surface first).
 *   - At most `maxHints` hints are returned (default 3).
 *
 * Pure function — no I/O, no mutation.
 */
export function matchCandidatesInIndex(
  task: Task,
  index: CandidateIndex,
  maxHints: number = DEFAULT_MAX_HINTS,
): readonly TaskHint[] {
  if (index.size === 0) return [];
  if (maxHints <= 0) return [];

  const taskKeywords = extractKeywords(
    `${task.title} ${task.labels.join(" ")}`,
  );
  if (taskKeywords.length === 0) return [];
  const taskKeywordSet = new Set(taskKeywords);

  interface Scored {
    readonly overlap: number;
    readonly capturedAt: string;
    readonly hint: TaskHint;
  }

  const scoredByCandidate = new Map<string, Scored>();

  for (const kw of taskKeywords) {
    const bucket = index.byKeyword.get(kw);
    if (bucket === undefined) continue;
    for (const candidate of bucket) {
      if (candidate.sourceTaskId === task.id) continue;
      if (scoredByCandidate.has(candidate.sourceTaskId)) continue;

      // Preserve candidate.keywords ordering in the matched array.
      const matched = candidate.keywords.filter((k) => taskKeywordSet.has(k));
      if (matched.length === 0) continue;

      scoredByCandidate.set(candidate.sourceTaskId, {
        overlap: matched.length,
        capturedAt: candidate.capturedAt,
        hint: {
          sourceTaskId: candidate.sourceTaskId,
          sourceType: candidate.sourceType,
          title: candidate.title,
          reason: candidate.reason,
          capturedAt: candidate.capturedAt,
          matchedKeywords: matched,
        },
      });
    }
  }

  if (scoredByCandidate.size === 0) return [];

  const scored = Array.from(scoredByCandidate.values());
  scored.sort((a, b) => {
    if (a.overlap !== b.overlap) return b.overlap - a.overlap;
    return b.capturedAt.localeCompare(a.capturedAt);
  });

  return scored.slice(0, maxHints).map((s) => s.hint);
}

/**
 * One-shot convenience wrapper: build an index and match in a single call.
 * Callers processing many tasks should build the index once themselves.
 */
export function matchCandidates(
  task: Task,
  candidates: readonly TaskCandidate[],
  maxHints: number = DEFAULT_MAX_HINTS,
): readonly TaskHint[] {
  return matchCandidatesInIndex(task, buildCandidateIndex(candidates), maxHints);
}
