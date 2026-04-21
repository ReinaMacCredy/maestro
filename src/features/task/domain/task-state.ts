import {
  claimedTaskCannotBeReopened,
  taskAlreadyCompleted,
  taskBlockedByOpenTasks,
  taskClaimBusySession,
  taskMutationOwnedByDifferentSession,
  taskMutationRequiresOwnershipContext,
  taskReasonRequiresCompletedStatus,
  taskStatusRequiresClaim,
} from "./task-errors.js";
import type {
  Task,
  TaskMutationInput,
  TaskStatus,
  UpdateTaskInput,
} from "./task-types.js";

export interface UpdateDecision {
  readonly nextStatus: TaskStatus;
  readonly autoClaim?: { readonly sessionId: string };
}

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
  actor: TaskMutationInput = {},
): UpdateDecision {
  if (existing.status === "completed") {
    throw taskAlreadyCompleted(existing.id);
  }
  assertTaskMutationOwnership(existing, actor, "update");
  if (patch.reason !== undefined && patch.status !== "completed") {
    throw taskReasonRequiresCompletedStatus();
  }

  const nextStatus = patch.status ?? existing.status;
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
  if (existing.assignee && patch.status === "pending" && existing.status !== "pending") {
    throw claimedTaskCannotBeReopened(existing.id);
  }
  if (!existing.assignee && nextStatus === "in_progress") {
    if (!actor.sessionId) {
      throw taskStatusRequiresClaim("in_progress");
    }
    const busy = findBusySessionTasks(actor.sessionId, existing.id, tasks);
    if (busy.length > 0) {
      throw taskClaimBusySession(actor.sessionId, busy);
    }
    return { nextStatus, autoClaim: { sessionId: actor.sessionId } };
  }

  return { nextStatus };
}

export function findBusySessionTasks(
  sessionId: string,
  excludeId: string,
  tasks: ReadonlyMap<string, Task>,
): readonly string[] {
  const busy: string[] = [];
  for (const task of tasks.values()) {
    if (task.id === excludeId) continue;
    if (task.status === "completed") continue;
    if (task.assignee === sessionId) {
      busy.push(task.id);
    }
  }
  return busy;
}

export function releaseTaskOwnership(task: Task, now: string): Task {
  return {
    ...task,
    assignee: undefined,
    claimedAt: undefined,
    lastActivityAt: undefined,
    status: task.status === "in_progress" ? "pending" : task.status,
    updatedAt: now,
  };
}

export function assertTaskMutationOwnership(
  task: Task,
  actor: TaskMutationInput,
  action: "update" | "block" | "unblock",
): void {
  if (!task.assignee || actor.force) {
    return;
  }
  if (!actor.sessionId) {
    throw taskMutationRequiresOwnershipContext(task.id, task.assignee, action);
  }
  if (task.assignee !== actor.sessionId) {
    throw taskMutationOwnedByDifferentSession(task.id, task.assignee, action);
  }
}
