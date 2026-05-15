/** Token-budget projection helpers. See `docs/token-budget.md`. */

import type { EvidenceRow, EvidenceSummary } from "@/features/evidence/domain/types.js";
import type { HandoffRecord, HandoffSummary } from "@/features/handoff/domain/handoff-types.js";
import type { Mission, MissionSummary } from "@/shared/domain/legacy-mission";
import type { LegacyTask as Task, TaskSummary } from "@/shared/domain/legacy-task";

export const PROJECTION_VIEWS = ["summary", "full"] as const;
export type ProjectionView = (typeof PROJECTION_VIEWS)[number];

export function summarizeTask(task: Task): TaskSummary {
  // `status` already signals open vs taken; agents that need the owner string
  // recover it with `--full` / `view: "full"` or `task get <id>`.
  return {
    ...(task.slug !== undefined ? { slug: task.slug } : {}),
    id: task.id,
    title: task.title,
    status: task.status,
    type: task.type,
    priority: task.priority,
    blockedByCount: task.blockedBy.length,
    ...(task.parentId !== undefined ? { parentId: task.parentId } : {}),
    ...(task.missionId !== undefined ? { missionId: task.missionId } : {}),
  };
}

export function summarizeMission(mission: Mission): MissionSummary {
  return {
    id: mission.id,
    title: mission.title,
    status: mission.status,
    milestoneCount: mission.milestones.length,
    featureCount: mission.features.length,
    updatedAt: mission.updatedAt,
  };
}

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

export function summarizeHandoff(record: HandoffRecord): HandoffSummary {
  return {
    name: record.name,
    id: record.id,
    status: record.status,
    task: record.task,
    agent: record.agent,
    model: record.model,
    createdAt: record.createdAt,
    wait: record.wait,
    ...(record.refs.taskId !== undefined ? { taskId: record.refs.taskId } : {}),
    ...(record.refs.missionId !== undefined
      ? { missionId: record.refs.missionId }
      : {}),
  };
}
