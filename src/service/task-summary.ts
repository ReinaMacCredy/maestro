import { TASK_STATES, type TaskState } from "../types/task-state.js";
import type { Task } from "../types/task.js";

export interface TaskSummary {
  readonly total: number;
  readonly byState: Record<TaskState, number>;
}

export function summarizeTasks(tasks: readonly Task[]): TaskSummary {
  const byState: Record<TaskState, number> = Object.fromEntries(
    TASK_STATES.map((s) => [s, 0]),
  ) as Record<TaskState, number>;
  for (const t of tasks) byState[t.state] += 1;
  return { total: tasks.length, byState };
}
