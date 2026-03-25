/**
 * Task state transitions and status predicates.
 * Extracted from TaskPort to keep the port definition pure (types only).
 */

import type { TaskStatusType } from '../../core/types.ts';

/**
 * Valid state transitions for task status (6-state model).
 *
 *   pending  --> claimed, blocked
 *   claimed  --> review, done, blocked, pending (release)
 *   review   --> done (accept), revision (reject)
 *   revision --> claimed (re-claim)
 *   blocked  --> pending (unblock)
 *   done     --> pending (reopen)
 */
export const VALID_TRANSITIONS: Record<TaskStatusType, TaskStatusType[]> = {
  pending: ['claimed', 'blocked'],
  claimed: ['review', 'done', 'blocked', 'pending'],
  review: ['done', 'revision'],
  revision: ['claimed'],
  blocked: ['pending'],
  done: ['pending'],
};

export function isValidTransition(from: TaskStatusType, to: TaskStatusType): boolean {
  return VALID_TRANSITIONS[from]?.includes(to) ?? false;
}

/** Task should not be overwritten by re-sync (any status other than pending/blocked). */
export function isActiveTask(status: TaskStatusType): boolean {
  return status === 'done' || status === 'claimed' || status === 'review' || status === 'revision';
}

/** A dependency is satisfied when the upstream task is done or in review. */
export function isDependencySatisfied(status: TaskStatusType): boolean {
  return status === 'done' || status === 'review';
}
