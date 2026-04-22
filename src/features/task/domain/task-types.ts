/**
 * Task domain types.
 * Modeled after `br` (beads_rust), adapted to maestro's style.
 *
 * Tasks are a lightweight, mutable issue graph that complements missions.
 * Missions answer "what are we building?"; tasks answer "what do I do next?".
 */

// ============================
// Status, type, priority
// ============================

export type TaskStatus =
  | "pending"
  | "in_progress"
  | "completed";

export type TaskType =
  | "task"
  | "bug"
  | "feature"
  | "epic"
  | "chore";

export type TaskPriority = 0 | 1 | 2 | 3 | 4;

export const TASK_STATUSES: readonly TaskStatus[] = [
  "pending",
  "in_progress",
  "completed",
] as const;

export const TASK_TYPES: readonly TaskType[] = [
  "task",
  "bug",
  "feature",
  "epic",
  "chore",
] as const;

export const TASK_PRIORITIES: readonly TaskPriority[] = [0, 1, 2, 3, 4] as const;

// ============================
// Core entity
// ============================

export interface TaskReceipt {
  readonly summary: string;
  readonly surprise?: string;
  readonly verifiedBy?: readonly string[];
  readonly capturedAt: string;
}

export interface Task {
  readonly id: string;
  readonly title: string;
  readonly description?: string;
  readonly type: TaskType;
  readonly priority: TaskPriority;
  readonly status: TaskStatus;
  readonly parentId?: string;
  readonly labels: readonly string[];
  readonly blocks: readonly string[];
  readonly blockedBy: readonly string[];
  readonly assignee?: string;
  readonly claimedAt?: string;
  readonly contractId?: string;
  readonly claimedAtCommit?: string;
  readonly lastActivityAt?: string;
  readonly closeReason?: string;
  readonly receipt?: TaskReceipt;
  readonly createdAt: string;
  readonly updatedAt: string;
}

// ============================
// Create / update inputs
// ============================

export interface CreateTaskInput {
  readonly title: string;
  readonly description?: string;
  readonly type?: TaskType;
  readonly priority?: TaskPriority;
  readonly parentId?: string;
  readonly labels?: readonly string[];
  readonly blockedBy?: readonly string[];
}

export interface UpdateTaskInput {
  readonly title?: string;
  readonly description?: string;
  readonly status?: TaskStatus;
  readonly reason?: string;
  readonly priority?: TaskPriority;
  readonly type?: TaskType;
  readonly parentId?: string;
  readonly addLabels?: readonly string[];
  readonly removeLabels?: readonly string[];
  readonly summary?: string;
  readonly surprise?: string;
  readonly verifiedBy?: readonly string[];
}

export interface BuildTaskReceiptInput {
  readonly nextStatus: TaskStatus;
  readonly capturedAt: string;
  readonly summary?: string;
  readonly surprise?: string;
  readonly verifiedBy?: readonly string[];
  readonly reasonFallback?: string;
}

export interface TaskMutationInput {
  readonly sessionId?: string;
  readonly force?: boolean;
}

export interface UpdateTaskResult {
  readonly task: Task;
  readonly autoClaimed: boolean;
}

export interface TaskMetadataPatch {
  readonly contractId?: string | null;
  readonly claimedAtCommit?: string | null;
}

// ============================
// Defaults
// ============================

export const DEFAULT_TASK_TYPE: TaskType = "task";
export const DEFAULT_TASK_PRIORITY: TaskPriority = 2;
export const DEFAULT_TASK_STATUS: TaskStatus = "pending";

/** Build a `Map<id, Task>` view of a task list for cross-entity lookups. */
export function indexTasksById(
  tasks: readonly Task[],
): ReadonlyMap<string, Task> {
  return new Map(tasks.map((t) => [t.id, t] as const));
}

// ============================
// Query / filter shapes
// ============================

export interface ListTasksFilters {
  readonly status?: TaskStatus;
  readonly priority?: TaskPriority;
  readonly type?: TaskType;
  readonly label?: string;
  readonly parentId?: string;
  readonly assignee?: string;
  readonly limit?: number;
}

export interface ReadyTasksFilters {
  readonly limit?: number;
  readonly label?: string;
  readonly priority?: TaskPriority;
  readonly type?: TaskType;
  readonly assignee?: string;
  readonly unassigned?: boolean;
}

export interface ClaimTaskInput {
  readonly sessionId: string;
  readonly force?: boolean;
  readonly checkBusy?: boolean;
}

export interface UnclaimTaskInput {
  readonly sessionId: string;
  readonly force?: boolean;
}

export function buildTaskReceipt(
  existingReceipt: TaskReceipt | undefined,
  input: BuildTaskReceiptInput,
): TaskReceipt | undefined {
  if (input.nextStatus !== "completed") {
    return existingReceipt;
  }

  const summary = nonEmpty(input.summary) ?? nonEmpty(input.reasonFallback);
  const surprise = nonEmpty(input.surprise);
  const verifiedBy = input.verifiedBy?.filter((name) => name.length > 0) ?? [];

  if (!summary && !surprise && verifiedBy.length === 0) {
    return existingReceipt;
  }

  return {
    summary: summary ?? "",
    ...(surprise ? { surprise } : {}),
    ...(verifiedBy.length > 0 ? { verifiedBy } : {}),
    capturedAt: input.capturedAt,
  };
}

function nonEmpty(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : undefined;
}
