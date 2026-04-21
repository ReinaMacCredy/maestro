import type { Task } from "../domain/task-types.js";
import type { TaskCandidate } from "../domain/task-candidate.js";
import type { CandidateStorePort } from "../ports/candidate-store.port.js";
import { extractKeywords } from "../domain/extract-keywords.js";

/**
 * Capture a just-closed task as a memory candidate.
 *
 * The candidate carries the extracted keywords from the task title and
 * close reason so future `task ready` queries can surface it as a hint
 * when keywords overlap with a newly unblocked task.
 *
 * Returns undefined when no candidate was captured. Three no-op cases:
 *   1. Task has no close reason (cannot surface useful guidance)
 *   2. Close reason is whitespace-only
 *   3. Keyword extractor returned no tokens (all stop words / too short)
 *
 * This is the ONE active-memory seed in phase 1. Phase 2 adds capture
 * paths for handoff blind spots, ratchet failures, and memory-correct
 * entries — they each become their own use case alongside this one and
 * write to the same candidate store.
 */
export async function captureTaskCandidate(
  store: CandidateStorePort,
  task: Task,
): Promise<TaskCandidate | undefined> {
  const reason = task.closeReason?.trim() ?? "";
  const summary = task.receipt?.summary.trim() ?? "";
  const surprise = task.receipt?.surprise?.trim() ?? "";
  if (reason.length === 0 && summary.length === 0 && surprise.length === 0) {
    return undefined;
  }

  const keywordSource = [task.title, reason, summary, surprise].filter((s) => s.length > 0).join(" ");
  const keywords = extractKeywords(keywordSource);
  if (keywords.length === 0) return undefined;

  const storedReason = reason.length > 0 ? reason : summary;

  return store.create({
    id: task.id,
    sourceTaskId: task.id,
    title: task.title,
    reason: storedReason,
    keywords,
  });
}
