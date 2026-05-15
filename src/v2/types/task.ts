import type { TaskState } from "./task-state.js";

export type TaskId = string;

export interface Task {
  readonly id: TaskId;
  readonly slug: string;
  readonly title: string;
  readonly state: TaskState;
  readonly spec_path?: string;
  readonly plan_id?: string;
  readonly assignee?: string;
  readonly claimed_at?: string;
  readonly pr_url?: string;
  readonly merged_at?: string;
  readonly blocked_by: readonly string[];
  readonly block_reason?: string;
  readonly abandon_reason?: string;
  // Set by task-claim when a heavy-mode spec triggers an auto-worktree.
  readonly worktree_path?: string;
  readonly created_at: string;
  readonly updated_at: string;
}

export function generateTaskId(): TaskId {
  const rand = Math.random().toString(36).slice(2, 8);
  return `tsk-${Date.now().toString(36)}-${rand}`;
}

export const TASK_ID_PATTERN = /^tsk-[a-z0-9]+-[a-z0-9]+$/;

/** v1 task IDs had the shape `tsk-<6 hex chars>`, e.g. `tsk-aabbcc`. */
export const LEGACY_TASK_ID_PATTERN = /^tsk-[0-9a-f]{6}$/;

export function isTaskId(value: unknown): value is TaskId {
  return typeof value === "string" && TASK_ID_PATTERN.test(value);
}

/** Matches either a v1 (6 hex) or v2 (`tsk-x-y`) task ID. */
export function isAnyTaskId(value: unknown): value is string {
  return (
    typeof value === "string"
    && (TASK_ID_PATTERN.test(value) || LEGACY_TASK_ID_PATTERN.test(value))
  );
}

/** Combined regex matching both v1 and v2 task ID shapes.
 *  Use in places that require a RegExp (e.g., assertSafeSegment). */
export const ANY_TASK_ID_PATTERN = /^tsk-([0-9a-f]{6}|[a-z0-9]+-[a-z0-9]+)$/;
