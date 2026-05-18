/** Token-budget projection helpers. See `docs/token-budget.md`. */

import type { EvidenceRow, EvidenceSummary } from "@/features/evidence/domain/types.js";
import type { Task } from "@/types/task.js";

export interface TaskSummary {
  readonly id: string;
  readonly slug: string;
  readonly title: string;
  readonly state: string;
  readonly mission_id?: string;
  readonly assignee?: string;
  readonly blocked_by_count: number;
}

export function summarizeTask(task: Task): TaskSummary {
  return {
    id: task.id,
    slug: task.slug,
    title: task.title,
    state: task.state,
    ...(task.mission_id !== undefined ? { mission_id: task.mission_id } : {}),
    ...(task.assignee !== undefined ? { assignee: task.assignee } : {}),
    blocked_by_count: task.blocked_by.length,
  };
}

export const PROJECTION_VIEWS = ["summary", "full"] as const;
export type ProjectionView = (typeof PROJECTION_VIEWS)[number];

export function summarizeEvidence(row: EvidenceRow): EvidenceSummary {
  return {
    id: row.id,
    task_id: row.task_id,
    kind: row.kind,
    witness_level: row.witness_level,
    created_at: row.created_at,
    ...(row.session_id !== undefined ? { session_id: row.session_id } : {}),
  };
}
