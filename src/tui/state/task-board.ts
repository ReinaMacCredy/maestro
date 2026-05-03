import { TASK_STATUSES, type TaskQueryPort, type TaskStatus } from "@/features/task";
import type { EvidenceStorePort } from "@/features/evidence";
import type { EvidenceSummary, TaskBoardItem, TaskBoardSnapshot } from "./screen-types.js";

export async function buildTaskBoard(
  taskStore?: TaskQueryPort,
  evidenceStore?: EvidenceStorePort,
): Promise<TaskBoardSnapshot | null> {
  if (!taskStore) return null;
  const tasks = await taskStore.all();
  if (tasks.length === 0) return null;

  const columns = Object.fromEntries(
    TASK_STATUSES.map((status) => [status, [] as TaskBoardItem[]]),
  ) as Record<TaskStatus, TaskBoardItem[]>;

  for (const task of tasks) {
    let evidenceCount = 0;
    let recentEvidence: readonly EvidenceSummary[] = [];

    if (evidenceStore !== undefined) {
      const allEvidence = await evidenceStore.list({ task_id: task.id });
      evidenceCount = allEvidence.length;
      // allEvidence is sorted ascending by created_at; reverse + slice 5 for most-recent-first
      recentEvidence = [...allEvidence].reverse().slice(0, 5).map((row) => ({
        id: row.id,
        kind: row.kind,
        witness_level: row.witness_level,
        created_at: row.created_at,
      }));
    }

    const item: TaskBoardItem = {
      id: task.id,
      title: task.title,
      status: task.status,
      priority: task.priority,
      assignee: task.assignee,
      labels: task.labels,
      blockedByCount: task.blockedBy.length,
      evidenceCount,
      recentEvidence,
    };
    columns[task.status]?.push(item);
  }

  for (const status of TASK_STATUSES) {
    columns[status]!.sort((a, b) => a.priority - b.priority);
  }

  return { columns, totalCount: tasks.length };
}
