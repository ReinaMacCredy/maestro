import type { Task } from "../domain/task-types.js";
import { extractKeywords } from "../domain/extract-keywords.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import { taskNotFound } from "../domain/task-errors.js";

export interface SimilarTaskMatch {
  readonly task: Task;
  readonly overlap: number;
  readonly matchedKeywords: readonly string[];
}

const DEFAULT_LIMIT = 5;

export async function findSimilarTasks(
  store: TaskQueryPort,
  targetId: string,
  limit: number = DEFAULT_LIMIT,
): Promise<readonly SimilarTaskMatch[]> {
  const target = await store.get(targetId);
  if (!target) {
    throw taskNotFound(targetId);
  }

  const targetKeywords = tokensFor(target);
  if (targetKeywords.size === 0) return [];

  const all = await store.all();
  const scored: SimilarTaskMatch[] = [];

  for (const task of all) {
    if (task.id === target.id) continue;
    const otherKeywords = tokensFor(task);
    if (otherKeywords.size === 0) continue;

    const matched: string[] = [];
    for (const kw of otherKeywords) {
      if (targetKeywords.has(kw)) {
        matched.push(kw);
      }
    }
    if (matched.length === 0) continue;

    scored.push({
      task,
      overlap: matched.length,
      matchedKeywords: matched,
    });
  }

  scored.sort((a, b) => {
    if (a.overlap !== b.overlap) return b.overlap - a.overlap;
    return b.task.updatedAt.localeCompare(a.task.updatedAt);
  });

  return limit > 0 ? scored.slice(0, limit) : scored;
}

function tokensFor(task: Task): ReadonlySet<string> {
  const parts: string[] = [task.title];
  if (task.closeReason) parts.push(task.closeReason);
  if (task.receipt?.summary) parts.push(task.receipt.summary);
  if (task.receipt?.surprise) parts.push(task.receipt.surprise);
  return new Set(extractKeywords(parts.join(" ")));
}
