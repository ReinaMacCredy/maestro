import type { Task } from "../domain/task-types.js";
import type {
  TaskCandidate,
  CandidateSourceType,
} from "../domain/task-candidate.js";
import { extractKeywords } from "../domain/extract-keywords.js";

/**
 * A hint attached to a ready task, sourced from a past candidate.
 * Phase 1 hints always have `sourceType: "task-close"` but the field is
 * preserved so phase 2 can add other signal sources without breaking
 * consumers.
 */
export interface TaskHint {
  readonly sourceTaskId: string;
  readonly sourceType: CandidateSourceType;
  readonly title: string;
  readonly reason: string;
  readonly capturedAt: string;
  readonly matchedKeywords: readonly string[];
}

const DEFAULT_MAX_HINTS = 3;

/**
 * Compute hints for a single ready task by overlapping its title and
 * labels against each candidate's stored keywords.
 *
 * Rules:
 *   - A candidate matches when at least one of its keywords appears in
 *     the target task's own extracted keywords.
 *   - A task never sees its own past close as a hint (self-exclusion by
 *     sourceTaskId — covers the edge case of reopening a task that was
 *     previously closed and then unblocked).
 *   - Ties are broken by capturedAt DESC (newer hints surface first).
 *   - At most `maxHints` hints are returned (default 3).
 *
 * This is a pure function — no I/O, no mutation, no dates. Call sites
 * should already have loaded candidates via CandidateStorePort.all().
 */
export function matchCandidates(
  task: Task,
  candidates: readonly TaskCandidate[],
  maxHints: number = DEFAULT_MAX_HINTS,
): readonly TaskHint[] {
  if (candidates.length === 0) return [];
  if (maxHints <= 0) return [];

  const taskKeywords = new Set(
    extractKeywords(`${task.title} ${task.labels.join(" ")}`),
  );
  if (taskKeywords.size === 0) return [];

  interface Scored {
    readonly overlap: number;
    readonly capturedAt: string;
    readonly hint: TaskHint;
  }

  const scored: Scored[] = [];
  for (const candidate of candidates) {
    // Self-exclusion: a task should not see its own past close as a hint.
    if (candidate.sourceTaskId === task.id) continue;

    const matched: string[] = [];
    for (const kw of candidate.keywords) {
      if (taskKeywords.has(kw)) matched.push(kw);
    }
    if (matched.length === 0) continue;

    scored.push({
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

  scored.sort((a, b) => {
    if (a.overlap !== b.overlap) return b.overlap - a.overlap;
    return b.capturedAt.localeCompare(a.capturedAt);
  });

  return scored.slice(0, maxHints).map((s) => s.hint);
}
