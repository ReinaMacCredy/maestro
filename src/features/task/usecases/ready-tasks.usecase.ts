import type { Task, ReadyTasksFilters } from "../domain/task-types.js";
import { indexTasksById } from "../domain/task-types.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import type { CandidateStorePort } from "../ports/candidate-store.port.js";
import {
  buildCandidateIndex,
  matchCandidatesInIndex,
  type TaskHint,
} from "./match-candidates.usecase.js";

const DEFAULT_LIMIT = 20;

export interface TaskBriefing extends Task {
  readonly hints: readonly TaskHint[];
}

export async function readyTasks(
  store: TaskQueryPort,
  filters: ReadyTasksFilters = {},
  _now: Date = new Date(),
  candidateStore?: CandidateStorePort,
): Promise<readonly TaskBriefing[]> {
  const all = await store.all();
  const byId = indexTasksById(all);

  const selected = all.filter((task) => {
    if (task.status !== "pending") return false;
    if (hasOpenBlockers(task, byId)) return false;

    if (filters.label !== undefined && !task.labels.includes(filters.label)) return false;
    if (filters.priority !== undefined && task.priority !== filters.priority) return false;
    if (filters.type !== undefined && task.type !== filters.type) return false;
    if (filters.assignee !== undefined && task.assignee !== filters.assignee) return false;
    if (filters.unassigned && task.assignee !== undefined) return false;
    if (filters.assignee === undefined && !filters.unassigned && task.assignee !== undefined) return false;

    return true;
  });

  selected.sort(hybridCompare);

  const limit = filters.limit ?? DEFAULT_LIMIT;
  const sliced = limit > 0 && selected.length > limit
    ? selected.slice(0, limit)
    : selected;

  if (sliced.length === 0) {
    return [];
  }

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

function hasOpenBlockers(
  task: Task,
  byId: ReadonlyMap<string, Task>,
): boolean {
  for (const blockerId of task.blockedBy) {
    const blocker = byId.get(blockerId);
    if (blocker && blocker.status !== "completed") {
      return true;
    }
  }
  return false;
}

function hybridCompare(a: Task, b: Task): number {
  const aHigh = a.priority <= 1 ? 0 : 1;
  const bHigh = b.priority <= 1 ? 0 : 1;
  if (aHigh !== bHigh) return aHigh - bHigh;
  return a.createdAt.localeCompare(b.createdAt);
}
