import type { Task, ReadyTasksFilters } from "../domain/task-types.js";
import { indexTasksById } from "../domain/task-types.js";
import { isTaskReady } from "../domain/task-state.js";
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

export interface ReadyTaskPage {
  readonly totalReady: number;
  readonly totalPending: number;
  readonly items: readonly Task[];
}

export async function readyTaskPage(
  store: TaskQueryPort,
  filters: ReadyTasksFilters = {},
): Promise<ReadyTaskPage> {
  const all = await store.all();
  return selectReadyTaskPage(all, filters);
}

export async function readyTasks(
  store: TaskQueryPort,
  filters: ReadyTasksFilters = {},
  _now: Date = new Date(),
  candidateStore?: CandidateStorePort,
): Promise<readonly TaskBriefing[]> {
  const page = await readyTaskPage(store, filters);
  if (page.items.length === 0) {
    return [];
  }

  const candidates = candidateStore ? await candidateStore.all() : [];
  if (candidates.length === 0) {
    return page.items.map((task) => ({ ...task, hints: [] as readonly TaskHint[] }));
  }
  const index = buildCandidateIndex(candidates);

  return page.items.map((task) => ({
    ...task,
    hints: matchCandidatesInIndex(task, index),
  }));
}

function selectReadyTaskPage(
  all: readonly Task[],
  filters: ReadyTasksFilters,
): ReadyTaskPage {
  const byId = indexTasksById(all);
  let totalPending = 0;

  const selected = all.filter((task) => {
    if (task.status !== "pending") return false;
    totalPending += 1;
    if (!isTaskReady(task, byId)) return false;

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
  const items = limit > 0 && selected.length > limit
    ? selected.slice(0, limit)
    : selected;

  return {
    totalReady: selected.length,
    totalPending,
    items,
  };
}

function hybridCompare(a: Task, b: Task): number {
  const aHigh = a.priority <= 1 ? 0 : 1;
  const bHigh = b.priority <= 1 ? 0 : 1;
  if (aHigh !== bHigh) return aHigh - bHigh;
  return a.createdAt.localeCompare(b.createdAt);
}
