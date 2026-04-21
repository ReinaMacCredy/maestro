import {
  TASK_STATUSES,
  TASK_TYPES,
  TASK_PRIORITIES,
  type Task,
  type TaskReceipt,
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
  taskBlockCycle,
  taskSelfBlock,
} from "./task-errors.js";
import { normalizeStoredTaskStatus } from "./task-state.js";

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
 * Validate a Task object loaded from storage. Returns the normalized task if
 * valid, undefined otherwise.
 *
 * Legacy task rows are accepted and normalized:
 * - open|blocked|deferred -> pending
 * - closed -> completed
 * - dependsOn -> blockedBy
 */
export function validateTask(value: unknown): Task | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const t = value as Record<string, unknown>;

  if (typeof t.id !== "string" || !TASK_ID_PATTERN.test(t.id)) return undefined;
  if (typeof t.title !== "string" || t.title.length === 0) return undefined;
  if (!isTaskType(t.type)) return undefined;
  if (!isTaskPriority(t.priority)) return undefined;
  const normalizedStatus = normalizeStoredStatus(t.status);
  if (!normalizedStatus) return undefined;
  if (!Array.isArray(t.labels)) return undefined;
  if (!t.labels.every((l) => typeof l === "string")) return undefined;

  const blocks = normalizeTaskIdArray(t.blocks);
  if (!blocks) return undefined;

  const blockedBy = normalizeBlockedBy(t);
  if (!blockedBy) return undefined;

  if (typeof t.createdAt !== "string") return undefined;
  if (typeof t.updatedAt !== "string") return undefined;

  if (t.description !== undefined && typeof t.description !== "string") return undefined;
  if (t.parentId !== undefined && typeof t.parentId !== "string") return undefined;
  if (t.assignee !== undefined && typeof t.assignee !== "string") return undefined;
  if (t.claimedAt !== undefined && typeof t.claimedAt !== "string") return undefined;
  if (t.contractId !== undefined && typeof t.contractId !== "string") return undefined;
  if (t.claimedAtCommit !== undefined && typeof t.claimedAtCommit !== "string") return undefined;
  if (t.lastActivityAt !== undefined && typeof t.lastActivityAt !== "string") return undefined;
  if (t.closeReason !== undefined && typeof t.closeReason !== "string") return undefined;

  let receipt: TaskReceipt | undefined;
  if (t.receipt !== undefined) {
    const parsed = normalizeStoredReceipt(t.receipt);
    if (parsed === null) return undefined;
    receipt = parsed;
  }

  return {
    id: t.id,
    title: t.title,
    description: t.description as string | undefined,
    type: t.type,
    priority: t.priority,
    status: normalizedStatus,
    parentId: t.parentId as string | undefined,
    labels: t.labels as readonly string[],
    blocks,
    blockedBy,
    assignee: t.assignee as string | undefined,
    claimedAt: t.claimedAt as string | undefined,
    contractId: t.contractId as string | undefined,
    claimedAtCommit: t.claimedAtCommit as string | undefined,
    lastActivityAt: t.lastActivityAt as string | undefined,
    closeReason: t.closeReason as string | undefined,
    receipt,
    createdAt: t.createdAt,
    updatedAt: t.updatedAt,
  };
}

function normalizeStoredReceipt(value: unknown): TaskReceipt | null {
  if (typeof value !== "object" || value === null) return null;
  const r = value as Record<string, unknown>;
  if (typeof r.summary !== "string") return null;
  if (typeof r.capturedAt !== "string") return null;
  if (r.surprise !== undefined && typeof r.surprise !== "string") return null;
  let verifiedBy: readonly string[] | undefined;
  if (r.verifiedBy !== undefined) {
    if (!Array.isArray(r.verifiedBy)) return null;
    if (!r.verifiedBy.every((item) => typeof item === "string")) return null;
    verifiedBy = r.verifiedBy as readonly string[];
  }
  return {
    summary: r.summary,
    surprise: r.surprise as string | undefined,
    verifiedBy,
    capturedAt: r.capturedAt,
  };
}

/**
 * Validate a CreateTaskInput. Throws MaestroError on invalid input.
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
  if (input.blockedBy !== undefined) {
    for (const blocker of input.blockedBy) {
      if (!TASK_ID_PATTERN.test(blocker)) {
        throw invalidTaskField("blocked-by", `'${blocker}' does not match ${TASK_ID_PATTERN}`);
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
    blockedBy: input.blockedBy,
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
  if (input.reason !== undefined && typeof input.reason !== "string") {
    throw invalidTaskField("reason", "must be a string");
  }
  if (input.summary !== undefined && typeof input.summary !== "string") {
    throw invalidTaskField("summary", "must be a string");
  }
  if (input.surprise !== undefined && typeof input.surprise !== "string") {
    throw invalidTaskField("surprise", "must be a string");
  }
  if (input.verifiedBy !== undefined) {
    for (const name of input.verifiedBy) {
      if (typeof name !== "string" || name.trim().length === 0) {
        throw invalidTaskField("verified-by", "each entry must be a non-empty string");
      }
    }
  }
  const hasReceiptField = input.summary !== undefined
    || input.surprise !== undefined
    || input.verifiedBy !== undefined;
  if (hasReceiptField && input.status !== "completed") {
    throw invalidTaskField(
      "summary",
      "--summary, --surprise, and --verified-by require --status completed",
    );
  }

  return {
    ...input,
    title: input.title?.trim(),
    reason: input.reason?.trim(),
    summary: input.summary?.trim(),
    surprise: input.surprise?.trim(),
    verifiedBy: input.verifiedBy?.map((name) => name.trim()).filter((name) => name.length > 0),
  };
}

export function validateBlockIds(blockedTaskIds: readonly string[]): readonly string[] {
  if (blockedTaskIds.length === 0) {
    throw invalidTaskField("block", "must include at least one task id");
  }

  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const taskId of blockedTaskIds) {
    if (!TASK_ID_PATTERN.test(taskId)) {
      throw invalidTaskField("block", `'${taskId}' does not match ${TASK_ID_PATTERN}`);
    }
    if (!seen.has(taskId)) {
      normalized.push(taskId);
      seen.add(taskId);
    }
  }

  return normalized;
}

/**
 * Walk the parent chain from `startId` and ensure `candidateParentId` does not
 * appear as an ancestor of `startId`, which would create a cycle.
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
    if (current === undefined) return;
    const currentTask = tasks.get(current);
    const parentId = currentTask?.parentId;
    if (parentId === undefined) return;
    if (parentId === startId) {
      chain.push(parentId);
      throw cyclicParent(startId, chain);
    }
    chain.push(parentId);
    current = parentId;
  }

  throw parentDepthExceeded(startId, MAX_PARENT_DEPTH);
}

export function assertNoBlockCycle(
  blockerId: string,
  blockedTaskIds: readonly string[],
  tasks: ReadonlyMap<string, Task>,
): void {
  for (const blockedTaskId of blockedTaskIds) {
    if (blockedTaskId === blockerId) {
      throw taskSelfBlock(blockerId);
    }
    const parents = new Map<string, string | undefined>([[blockedTaskId, blockerId]]);
    const stack: string[] = [blockedTaskId];

    while (stack.length > 0) {
      const currentId = stack.pop();
      if (!currentId) {
        continue;
      }
      const current = tasks.get(currentId);
      if (!current) {
        continue;
      }

      for (const nextBlockedId of current.blocks) {
        if (nextBlockedId === blockerId) {
          throw taskBlockCycle(blockerId, buildBlockCycleChain(blockerId, currentId, parents));
        }
        if (parents.has(nextBlockedId)) {
          continue;
        }
        parents.set(nextBlockedId, currentId);
        stack.push(nextBlockedId);
      }
    }
  }
}

function buildBlockCycleChain(
  blockerId: string,
  currentId: string,
  parents: ReadonlyMap<string, string | undefined>,
): readonly string[] {
  const chain: string[] = [currentId];
  let cursor = currentId;

  while (true) {
    const parentId = parents.get(cursor);
    if (parentId === undefined) {
      break;
    }
    chain.push(parentId);
    if (parentId === blockerId) {
      break;
    }
    cursor = parentId;
  }

  chain.reverse();
  chain.push(blockerId);
  return chain;
}

function normalizeStoredStatus(value: unknown): TaskStatus | undefined {
  return normalizeStoredTaskStatus(value);
}

function normalizeTaskIdArray(value: unknown): readonly string[] | undefined {
  if (value === undefined) return [];
  if (!Array.isArray(value)) return undefined;
  if (!value.every((item) => typeof item === "string")) return undefined;
  return value;
}

function normalizeBlockedBy(value: Record<string, unknown>): readonly string[] | undefined {
  const blockedBy = normalizeTaskIdArray(value.blockedBy);
  if (!blockedBy) return undefined;

  if (value.dependsOn === undefined) {
    return blockedBy;
  }

  const legacyDependsOn = normalizeTaskIdArray(value.dependsOn);
  if (!legacyDependsOn) return undefined;

  const seen = new Set<string>(blockedBy);
  const normalized = [...blockedBy];
  for (const depId of legacyDependsOn) {
    if (!seen.has(depId)) {
      normalized.push(depId);
      seen.add(depId);
    }
  }
  return normalized;
}
