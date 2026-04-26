import { TASK_STATUSES, type TaskQueryPort, type TaskStatus } from "@/features/task";
import type { TaskBoardItem, TaskBoardSnapshot } from "./screen-types.js";

export async function buildTaskBoard(
  taskStore?: TaskQueryPort,
): Promise<TaskBoardSnapshot | null> {
  if (!taskStore) return null;
  const tasks = await taskStore.all();
  if (tasks.length === 0) return null;

  const columns = Object.fromEntries(
    TASK_STATUSES.map((status) => [status, [] as TaskBoardItem[]]),
  ) as Record<TaskStatus, TaskBoardItem[]>;

  for (const task of tasks) {
    const item: TaskBoardItem = {
      id: task.id,
      title: task.title,
      status: task.status,
      priority: task.priority,
      assignee: task.assignee,
      labels: task.labels,
      blockedByCount: task.blockedBy.length,
    };
    columns[task.status]?.push(item);
  }

  for (const status of TASK_STATUSES) {
    columns[status]!.sort((a, b) => a.priority - b.priority);
  }

  return { columns, totalCount: tasks.length };
}
