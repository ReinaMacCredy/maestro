import { MaestroError } from "@/shared/errors.js";
import {
  TASK_PRIORITIES,
  TASK_STATUSES,
  TASK_TYPES,
  type CreateTaskInput,
  type TaskPriority,
  type TaskStatus,
  type TaskType,
  type UpdateTaskInput,
} from "../domain/task-types.js";
import { isTaskPriority, isTaskStatus, isTaskType } from "../domain/task-validators.js";
import { isLegacyTaskStatus } from "../domain/task-state.js";
import {
  batchInvalidJson,
  batchMalformedInput,
  taskCompletedViaUpdateStatus,
  taskCreateCompletedRejected,
} from "../domain/task-errors.js";
import type { BatchInput } from "../domain/task-batch-types.js";

export interface CreateOpts {
  description?: string;
  type?: string;
  priority?: string;
  parent?: string;
  labels?: string;
  blockedBy?: string;
}

export function buildCreateInput(title: string, opts: CreateOpts): CreateTaskInput {
  return {
    title,
    description: opts.description,
    type: parseType(opts.type),
    priority: parsePriority(opts.priority),
    parentId: opts.parent,
    labels: parseList(opts.labels),
    blockedBy: parseList(opts.blockedBy),
  };
}

export function parseType(value: string | undefined): TaskType | undefined {
  if (value === undefined) return undefined;
  if (isTaskType(value)) {
    return value;
  }
  throw new MaestroError(`Invalid --type '${value}'`, [
    `Valid types: ${TASK_TYPES.join(", ")}`,
  ]);
}

export function parseStatus(value: string | undefined): TaskStatus | undefined {
  if (value === undefined) return undefined;
  if (isTaskStatus(value)) {
    return value;
  }
  if (isLegacyTaskStatus(value)) {
    if (value === "closed") {
      throw taskCompletedViaUpdateStatus();
    }
    throw new MaestroError(`Legacy --status '${value}' is no longer supported`, [
      "Use one of: pending, in_progress, completed",
      "Use 'maestro task claim <id>' before moving work to in_progress",
      "Use 'maestro task update <id> --status completed --reason \"...\"' to finish work",
    ]);
  }
  throw new MaestroError(`Invalid --status '${value}'`, [
    `Valid statuses: ${TASK_STATUSES.join(", ")}`,
  ]);
}

export function parseCreateStatus(
  value: string | undefined,
): Exclude<TaskStatus, "completed"> | undefined {
  if (value === undefined) return undefined;
  const parsed = parseStatus(value);
  if (parsed === "completed") {
    throw taskCreateCompletedRejected();
  }
  return parsed;
}

export function parseLimit(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  const n = parseWholeNumber(value);
  if (n === undefined) {
    throw new MaestroError(`Invalid --limit '${value}'`, [
      "Limit must be a non-negative integer (0 = unlimited)",
    ]);
  }
  return n;
}

export function hasAnyPatchField(patch: UpdateTaskInput): boolean {
  return (
    patch.title !== undefined ||
    patch.description !== undefined ||
    patch.status !== undefined ||
    patch.reason !== undefined ||
    patch.priority !== undefined ||
    patch.type !== undefined ||
    patch.parentId !== undefined ||
    (patch.addLabels !== undefined && patch.addLabels.length > 0) ||
    (patch.removeLabels !== undefined && patch.removeLabels.length > 0) ||
    patch.summary !== undefined ||
    patch.surprise !== undefined ||
    (patch.verifiedBy !== undefined && patch.verifiedBy.length > 0)
  );
}

export function parsePriority(value: string | undefined): TaskPriority | undefined {
  if (value === undefined) return undefined;
  const n = parseWholeNumber(value);
  if (n === undefined || !isTaskPriority(n)) {
    throw new MaestroError(`Invalid --priority '${value}'`, [
      `Priority must be one of ${TASK_PRIORITIES.join(", ")}`,
      "0 = critical, 4 = backlog",
    ]);
  }
  return n;
}

function parseWholeNumber(value: string): number | undefined {
  if (!/^\d+$/.test(value)) {
    return undefined;
  }

  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : undefined;
}

export function parseList(value: string | undefined): readonly string[] | undefined {
  if (value === undefined) return undefined;
  return value
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

export function parsePlanInput(raw: string): BatchInput {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw batchInvalidJson(detail);
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw batchMalformedInput("plan must be a JSON object");
  }
  const obj = parsed as Record<string, unknown>;
  if (obj.batchId !== undefined && typeof obj.batchId !== "string") {
    throw batchMalformedInput("'batchId' must be a string when provided");
  }
  if (!Array.isArray(obj.tasks)) {
    throw batchMalformedInput("'tasks' must be an array");
  }
  return {
    batchId: obj.batchId as string | undefined,
    tasks: obj.tasks as BatchInput["tasks"],
  };
}
