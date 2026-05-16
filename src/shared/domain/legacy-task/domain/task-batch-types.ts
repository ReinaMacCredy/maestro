import type {
  CreateTaskInput,
  Task,
  TaskPriority,
  TaskStatus,
  TaskType,
} from "./task-types.js";

/**
 * One task entry inside a batch plan. Mirrors CreateTaskInput but allows
 * `parent` / `blockedBy` to reference other members of the same batch by the
 * optional local `name` slot or a top-level task slug. Resolution is by shape:
 * a string matching TASK_ID_PATTERN is always treated as a real task id; any
 * other string must match exactly one batch-local name or slug. Name-slot
 * values that happen to match TASK_ID_PATTERN are rejected at parse time.
 */
export interface BatchTaskInput {
  readonly name?: string;
  readonly title: string;
  readonly description?: string;
  readonly type?: TaskType;
  readonly priority?: TaskPriority;
  readonly parent?: string;
  /**
   * Optional explicit slug for top-level entries. Mandatory at plan-conversion
   * (auto-derived from the title when omitted on a top-level entry). Forbidden
   * when `parent` is set.
   */
  readonly slug?: string;
  readonly labels?: readonly string[];
  readonly blockedBy?: readonly string[];
}

export interface BatchInput {
  readonly batchId?: string;
  readonly tasks: readonly BatchTaskInput[];
}

/**
 * Adapter-facing shape: references are already pre-resolved into either a real
 * task id (string) or a numeric zero-based index into the batch array.
 * Numeric indices become generated ids inside the adapter's locked write.
 */
export interface CreateBatchInput {
  readonly title: string;
  readonly description?: string;
  readonly type?: TaskType;
  readonly priority?: TaskPriority;
  readonly labels?: readonly string[];
  readonly parentRef?: number | string;
  readonly slug?: string;
  readonly blockedByRefs?: readonly (number | string)[];
}

export interface BatchCreatedTask {
  readonly name?: string;
  readonly id: string;
  readonly status: TaskStatus;
  readonly assignee?: string;
}

export interface BatchResult {
  readonly batchId?: string;
  readonly created: readonly BatchCreatedTask[];
  /**
   * True when the result was served from a stored receipt (idempotent replay)
   * rather than freshly created. Lets callers distinguish "created N tasks"
   * from "N tasks already exist from a prior batch submission".
   */
  readonly replayed?: boolean;
}

/** Re-export to keep batch-type consumers from reaching across modules. */
export type { CreateTaskInput, Task };
