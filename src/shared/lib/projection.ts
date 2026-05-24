/** Token-budget projection helpers. See `docs/token-budget.md`. */

import type { EvidenceRow, EvidenceSummary } from "@/features/evidence/domain/types.js";
import type { HandoffEnvelope } from "@/repo/handoff-emitter.port.js";
import type { Task, TaskId } from "@/types/task.js";

export interface TaskSummary {
  readonly id: string;
  readonly slug: string;
  readonly title: string;
  readonly state: string;
  readonly mission_id?: string;
  readonly assignee?: string;
  readonly parent_id?: TaskId;
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
    ...(task.parent_id !== undefined ? { parent_id: task.parent_id } : {}),
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

export interface HandoffSummary {
  readonly id: string;
  readonly task_id: string;
  readonly trigger_verb: string;
  readonly to_agent?: string;
  readonly created_at: string;
  readonly picked_up: boolean;
}

export function summarizeHandoff(
  envelope: HandoffEnvelope,
  pickedUp: boolean,
): HandoffSummary {
  return {
    id: envelope.id,
    task_id: envelope.task_id,
    trigger_verb: envelope.trigger_verb,
    ...(envelope.to_agent !== undefined ? { to_agent: envelope.to_agent } : {}),
    created_at: envelope.created_at,
    picked_up: pickedUp,
  };
}
