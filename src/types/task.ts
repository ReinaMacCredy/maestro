import type { TaskState } from "./task-state.js";

export type TaskId = string;

export interface Task {
  readonly id: TaskId;
  readonly slug: string;
  readonly title: string;
  readonly state: TaskState;
  readonly spec_path?: string;
  readonly mission_id?: string;
  readonly assignee?: string;
  readonly claimed_at?: string;
  readonly pr_url?: string;
  readonly merged_at?: string;
  readonly blocked_by: readonly string[];
  readonly parent_id?: TaskId;
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

export function isTaskId(value: unknown): value is TaskId {
  return typeof value === "string" && TASK_ID_PATTERN.test(value);
}

/** Regex matching both the original 6-hex shape and the current `tsk-x-y`
 *  shape. Used in places that require a RegExp (e.g., assertSafeSegment).
 *  The 6-hex alternation can be dropped once legacy task adapters stop
 *  reading older on-disk artifacts. */
export const ANY_TASK_ID_PATTERN = /^tsk-([0-9a-f]{6}|[a-z0-9]+-[a-z0-9]+)$/;
