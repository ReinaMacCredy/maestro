import {
  claimedTaskCannotBeReopened,
  taskAlreadyCompleted,
  taskBlockedByOpenTasks,
  taskReasonRequiresCompletedStatus,
  taskStatusRequiresClaim,
} from "./task-errors.js";
import type {
  Task,
  TaskStatus,
  UpdateTaskInput,
} from "./task-types.js";

export const LEGACY_TASK_STATUSES = [
  "open",
  "blocked",
  "deferred",
  "closed",
] as const;

const LEGACY_TASK_STATUS_SET = new Set<string>(LEGACY_TASK_STATUSES);

export function normalizeStoredTaskStatus(value: unknown): TaskStatus | undefined {
  if (value === "pending" || value === "in_progress" || value === "completed") {
    return value;
  }
  if (typeof value !== "string" || !LEGACY_TASK_STATUS_SET.has(value)) {
    return undefined;
  }

  switch (value) {
    case "open":
    case "blocked":
    case "deferred":
      return "pending";
    case "closed":
      return "completed";
    default:
      return undefined;
  }
}

export function isLegacyTaskStatus(value: unknown): value is typeof LEGACY_TASK_STATUSES[number] {
  return typeof value === "string" && LEGACY_TASK_STATUS_SET.has(value);
}

export function getUnresolvedBlockerIds(
  task: Task,
  tasks: ReadonlyMap<string, Task>,
): readonly string[] {
  return task.blockedBy.filter((blockerId) => {
    const blocker = tasks.get(blockerId);
    return blocker === undefined || blocker.status !== "completed";
  });
}

export function hasUnresolvedBlockers(
  task: Task,
  tasks: ReadonlyMap<string, Task>,
): boolean {
  return getUnresolvedBlockerIds(task, tasks).length > 0;
}

export function assertTaskUpdateAllowed(
  existing: Task,
  patch: UpdateTaskInput,
  tasks: ReadonlyMap<string, Task>,
): TaskStatus {
  if (existing.status === "completed") {
    throw taskAlreadyCompleted(existing.id);
  }
  if (patch.reason !== undefined && patch.status !== "completed") {
    throw taskReasonRequiresCompletedStatus();
  }

  const nextStatus = patch.status ?? existing.status;
  if (!existing.assignee && nextStatus === "in_progress") {
    throw taskStatusRequiresClaim("in_progress");
  }
  if (existing.assignee && nextStatus === "pending") {
    throw claimedTaskCannotBeReopened(existing.id);
  }
  if (
    patch.status !== undefined &&
    patch.status !== existing.status &&
    (nextStatus === "in_progress" || nextStatus === "completed")
  ) {
    const blockers = getUnresolvedBlockerIds(existing, tasks);
    if (blockers.length > 0) {
      throw taskBlockedByOpenTasks(existing.id, blockers);
    }
  }

  return nextStatus;
}

export function releaseTaskOwnership(task: Task, now: string): Task {
  return {
    ...task,
    assignee: undefined,
    claimedAt: undefined,
    status: task.status === "in_progress" ? "pending" : task.status,
    updatedAt: now,
  };
}
