import type { Task, ReadyTasksFilters } from "../domain/task-types.js";
import { indexTasksById } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import type { CandidateStorePort } from "../ports/candidate-store.port.js";
import {
  buildCandidateIndex,
  matchCandidatesInIndex,
  type TaskHint,
} from "./match-candidates.usecase.js";

const DEFAULT_LIMIT = 20;

/**
 * A Task enriched with computed hints for `task ready` output.
 *
 * hints is always present (empty array when no candidate store is
 * passed, or when no candidates match). This keeps the return type
 * stable across callers whether or not active-memory is enabled.
 */
export interface TaskBriefing extends Task {
  readonly hints: readonly TaskHint[];
}

/**
 * Ready query: unblocked, actionable tasks, each enriched with hints
 * from the candidate pool when available.
 *
 * Algorithm (copied from br, adapted to in-memory walk):
 *  1. Exclude closed tasks.
 *  2. Exclude tasks with any open blocking dependency (walk dependsOn
 *     recursively; parentId is NOT a blocking edge — it is only hierarchy).
 *  3. Exclude deferred tasks (deferUntil > now) unless --include-deferred.
 *  4. Apply user filters: label, priority, type, assignee, unassigned.
 *  5. Sort by hybrid: P0/P1 first by createdAt ASC, then everything else
 *     by createdAt ASC.
 *  6. Slice to --limit (default 20, 0 = unlimited).
 *  7. If candidateStore is provided, attach hints via matchCandidates.
 */
export async function readyTasks(
  store: TaskStorePort,
  filters: ReadyTasksFilters = {},
  now: Date = new Date(),
  candidateStore?: CandidateStorePort,
): Promise<readonly TaskBriefing[]> {
  const all = await store.all();
  const byId = indexTasksById(all);
  const nowIso = now.toISOString();

  const selected = all.filter((task) => {
    // Step 1: exclude closed.
    if (task.status === "closed") return false;

    // Step 2: exclude tasks blocked by any open dependency (transitive walk).
    if (hasOpenBlockingDependency(task, byId)) return false;

    // Step 3: exclude deferred unless overridden.
    if (
      !filters.includeDeferred &&
      task.deferUntil !== undefined &&
      task.deferUntil > nowIso
    ) {
      return false;
    }

    // Step 4: apply user filters.
    if (filters.label !== undefined && !task.labels.includes(filters.label)) return false;
    if (filters.priority !== undefined && task.priority !== filters.priority) return false;
    if (filters.type !== undefined && task.type !== filters.type) return false;
    if (filters.assignee !== undefined && task.assignee !== filters.assignee) return false;
    if (filters.unassigned && task.assignee !== undefined) return false;

    return true;
  });

  // Step 5: hybrid sort.
  selected.sort(hybridCompare);

  // Step 6: limit.
  const limit = filters.limit ?? DEFAULT_LIMIT;
  const sliced = limit > 0 && selected.length > limit
    ? selected.slice(0, limit)
    : selected;

  // Step 7: attach hints from candidate pool if available. Build the
  // keyword -> candidates index once and reuse it for every ready task.
  const candidates = candidateStore ? await candidateStore.all() : [];
  if (candidates.length === 0) {
    return sliced.map((task) => ({ ...task, hints: [] as readonly TaskHint[] }));
  }
  const index = buildCandidateIndex(candidates);

  return sliced.map((task) => ({
    ...task,
    hints: matchCandidatesInIndex(task, index),
  }));
}

/**
 * A task is blocked if any of its dependencies is not closed.
 * Walks dependsOn transitively with a cycle guard, but we only need to
 * know "is there ANY open dependency" — not the full set — so the first
 * open hit returns true.
 *
 * Missing dependency ids are treated as "not blocking" (orphaned edge)
 * — br treats them similarly because the referenced task may have been
 * hard-deleted.
 */
function hasOpenBlockingDependency(
  task: Task,
  byId: ReadonlyMap<string, Task>,
): boolean {
  if (task.dependsOn.length === 0) return false;

  const visited = new Set<string>([task.id]);
  const stack: string[] = [...task.dependsOn];

  while (stack.length > 0) {
    const depId = stack.pop() as string;
    if (visited.has(depId)) continue;
    visited.add(depId);

    const dep = byId.get(depId);
    if (!dep) continue; // orphaned edge; skip
    if (dep.status !== "closed") {
      return true; // blocked by an open ancestor in the dep chain
    }
  }

  return false;
}

/**
 * Hybrid sort: P0 and P1 tasks come first (still sorted by createdAt ASC
 * among themselves), then P2+ tasks (sorted by createdAt ASC).
 */
function hybridCompare(a: Task, b: Task): number {
  const aHigh = a.priority <= 1 ? 0 : 1;
  const bHigh = b.priority <= 1 ? 0 : 1;
  if (aHigh !== bHigh) return aHigh - bHigh;
  return a.createdAt.localeCompare(b.createdAt);
}
