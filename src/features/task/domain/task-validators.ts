import {
  TASK_STATUSES,
  TASK_TYPES,
  TASK_PRIORITIES,
  type Task,
  type TaskStatus,
  type TaskType,
  type TaskPriority,
  type CreateTaskInput,
  type UpdateTaskInput,
} from "./task-types.js";
import { TASK_ID_PATTERN } from "./task-id.js";
import {
  invalidTaskField,
  cyclicParent,
  parentDepthExceeded,
} from "./task-errors.js";

const MAX_PARENT_DEPTH = 32;

export function isTaskStatus(value: unknown): value is TaskStatus {
  return typeof value === "string" && (TASK_STATUSES as readonly string[]).includes(value);
}

export function isTaskType(value: unknown): value is TaskType {
  return typeof value === "string" && (TASK_TYPES as readonly string[]).includes(value);
}

export function isTaskPriority(value: unknown): value is TaskPriority {
  return typeof value === "number" && (TASK_PRIORITIES as readonly number[]).includes(value);
}

/**
 * Validate a Task object loaded from storage. Returns the task if valid,
 * undefined otherwise. Matches the `validateMission` pattern.
 */
export function validateTask(value: unknown): Task | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const t = value as Record<string, unknown>;

  if (typeof t.id !== "string" || !TASK_ID_PATTERN.test(t.id)) return undefined;
  if (typeof t.title !== "string" || t.title.length === 0) return undefined;
  if (!isTaskType(t.type)) return undefined;
  if (!isTaskPriority(t.priority)) return undefined;
  if (!isTaskStatus(t.status)) return undefined;
  if (!Array.isArray(t.labels)) return undefined;
  if (!t.labels.every((l) => typeof l === "string")) return undefined;
  if (!Array.isArray(t.dependsOn)) return undefined;
  if (!t.dependsOn.every((d) => typeof d === "string")) return undefined;
  if (typeof t.createdAt !== "string") return undefined;
  if (typeof t.updatedAt !== "string") return undefined;

  if (t.description !== undefined && typeof t.description !== "string") return undefined;
  if (t.parentId !== undefined && typeof t.parentId !== "string") return undefined;
  if (t.assignee !== undefined && typeof t.assignee !== "string") return undefined;
  if (t.deferUntil !== undefined && typeof t.deferUntil !== "string") return undefined;
  if (t.closeReason !== undefined && typeof t.closeReason !== "string") return undefined;

  return {
    id: t.id,
    title: t.title,
    description: t.description as string | undefined,
    type: t.type,
    priority: t.priority,
    status: t.status,
    parentId: t.parentId as string | undefined,
    labels: t.labels as readonly string[],
    dependsOn: t.dependsOn as readonly string[],
    assignee: t.assignee as string | undefined,
    deferUntil: t.deferUntil as string | undefined,
    closeReason: t.closeReason as string | undefined,
    createdAt: t.createdAt,
    updatedAt: t.updatedAt,
  };
}

/**
 * Validate a CreateTaskInput. Throws MaestroError on invalid input.
 * Sanitized inputs are returned for downstream use-case consumption.
 */
export function validateCreateInput(input: CreateTaskInput): CreateTaskInput {
  if (typeof input.title !== "string" || input.title.trim().length === 0) {
    throw invalidTaskField("title", "must be a non-empty string");
  }
  if (input.type !== undefined && !isTaskType(input.type)) {
    throw invalidTaskField("type", `must be one of ${TASK_TYPES.join(", ")}`);
  }
  if (input.priority !== undefined && !isTaskPriority(input.priority)) {
    throw invalidTaskField("priority", `must be one of ${TASK_PRIORITIES.join(", ")}`);
  }
  if (input.parentId !== undefined && !TASK_ID_PATTERN.test(input.parentId)) {
    throw invalidTaskField("parent", `must match ${TASK_ID_PATTERN}`);
  }
  if (input.dependsOn !== undefined) {
    for (const dep of input.dependsOn) {
      if (!TASK_ID_PATTERN.test(dep)) {
        throw invalidTaskField("depends-on", `'${dep}' does not match ${TASK_ID_PATTERN}`);
      }
    }
  }
  if (input.labels !== undefined) {
    for (const label of input.labels) {
      if (typeof label !== "string" || label.length === 0) {
        throw invalidTaskField("label", "must be a non-empty string");
      }
    }
  }

  return {
    title: input.title.trim(),
    description: input.description,
    type: input.type,
    priority: input.priority,
    parentId: input.parentId,
    labels: input.labels,
    dependsOn: input.dependsOn,
    assignee: input.assignee,
  };
}

/**
 * Validate an UpdateTaskInput. Throws MaestroError on invalid input.
 */
export function validateUpdateInput(input: UpdateTaskInput): UpdateTaskInput {
  if (input.title !== undefined && (typeof input.title !== "string" || input.title.trim().length === 0)) {
    throw invalidTaskField("title", "must be a non-empty string");
  }
  if (input.status !== undefined && !isTaskStatus(input.status)) {
    throw invalidTaskField("status", `must be one of ${TASK_STATUSES.join(", ")}`);
  }
  if (input.priority !== undefined && !isTaskPriority(input.priority)) {
    throw invalidTaskField("priority", `must be one of ${TASK_PRIORITIES.join(", ")}`);
  }
  if (input.type !== undefined && !isTaskType(input.type)) {
    throw invalidTaskField("type", `must be one of ${TASK_TYPES.join(", ")}`);
  }
  if (input.parentId !== undefined && input.parentId !== "" && !TASK_ID_PATTERN.test(input.parentId)) {
    throw invalidTaskField("parent", `must match ${TASK_ID_PATTERN} or be empty`);
  }

  return {
    ...input,
    title: input.title?.trim(),
  };
}

/**
 * Walk the parent chain from `startId` and ensure `candidateParentId` does not
 * appear as an ancestor of `startId`, which would create a cycle.
 *
 * Throws cyclicParent if a cycle would be introduced.
 * Throws parentDepthExceeded if the chain is longer than MAX_PARENT_DEPTH.
 */
export function assertNoParentCycle(
  startId: string,
  candidateParentId: string,
  tasks: ReadonlyMap<string, Task>,
): void {
  if (startId === candidateParentId) {
    throw cyclicParent(startId, [startId, candidateParentId]);
  }

  const chain: string[] = [candidateParentId];
  let current: string | undefined = candidateParentId;

  for (let depth = 0; depth < MAX_PARENT_DEPTH; depth++) {
    const parent = tasks.get(current)?.parentId;
    if (parent === undefined) return;
    if (parent === startId) {
      chain.push(parent);
      throw cyclicParent(startId, chain);
    }
    chain.push(parent);
    current = parent;
  }

  throw parentDepthExceeded(startId, MAX_PARENT_DEPTH);
}
