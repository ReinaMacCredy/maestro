import { getUnresolvedBlockerIds } from "../domain/task-state.js";
import { indexTasksById } from "../domain/task-types.js";
import { taskNotFound } from "../domain/task-errors.js";
import type { Task } from "../domain/task-types.js";
import type { TaskContinuationEvent, TaskContinuationSummary } from "../domain/task-continuation-types.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import { loadTaskContinuationSummary } from "./task-continuation.usecase.js";

export interface TaskInspectionDeps {
  readonly taskStore: TaskQueryPort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
  readonly listOpenHandoffIds?: (taskId: string) => Promise<readonly string[]>;
}

export interface TaskInspectionView {
  readonly task: Task;
  readonly continuation?: TaskContinuationSummary;
  readonly recentEvents: readonly TaskContinuationEvent[];
  readonly steps?: readonly Task[];
  readonly activeBlockerIds: readonly string[];
  readonly openHandoffs: readonly string[];
}

export async function inspectTask(
  deps: TaskInspectionDeps,
  id: string,
): Promise<TaskInspectionView> {
  const task = await deps.taskStore.get(id);
  if (!task) {
    throw taskNotFound(id);
  }

  const [continuation, allTasks, openHandoffs] = await Promise.all([
    loadTaskContinuationSummary(deps.continuationStore, id),
    deps.taskStore.all(),
    deps.listOpenHandoffIds?.(id) ?? Promise.resolve([]),
  ]);
  const recentEvents = continuation
    ? await deps.continuationHistory.listRecent(id, 5)
    : [];
  const steps = task.parentId === undefined
    ? allTasks.filter((candidate) => candidate.parentId === id)
    : undefined;
  const activeBlockerIds = getUnresolvedBlockerIds(task, indexTasksById(allTasks));

  return {
    task,
    continuation,
    recentEvents,
    ...(steps !== undefined ? { steps } : {}),
    activeBlockerIds,
    openHandoffs,
  };
}
