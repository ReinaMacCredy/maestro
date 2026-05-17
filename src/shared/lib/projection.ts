/** Token-budget projection helpers. See `docs/token-budget.md`. */

import type { EvidenceRow, EvidenceSummary } from "@/features/evidence/domain/types.js";
import type { Mission, MissionSummary } from "@/shared/domain/legacy-mission";
import type { LegacyTask as Task, TaskSummary } from "@/shared/domain/legacy-task";
import type { Task as V2Task } from "@/types/task.js";

export interface V2TaskSummary {
  readonly id: string;
  readonly slug: string;
  readonly title: string;
  readonly state: string;
  readonly mission_id?: string;
  readonly assignee?: string;
  readonly blocked_by_count: number;
}

export function summarizeV2Task(task: V2Task): V2TaskSummary {
  // Detail fields (created_at, updated_at, spec_path, pr_url, merged_at,
  // claimed_at, worktree_path, block_reason, abandon_reason) live behind
  // `--full` / `view: "full"` or recover via `task get <id>`.
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

