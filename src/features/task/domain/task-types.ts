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
  readonly closeReason?: string;
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
}

export interface TaskMutationInput {
  readonly sessionId?: string;
  readonly force?: boolean;
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
