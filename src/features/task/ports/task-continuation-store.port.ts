import type { TaskContinuationSummary } from "../domain/task-continuation-types.js";

export interface TaskContinuationQueryPort {
  getActive(taskId: string): Promise<TaskContinuationSummary | undefined>;
  getCompleted(taskId: string): Promise<TaskContinuationSummary | undefined>;
  listActive(): Promise<readonly TaskContinuationSummary[]>;
  listCompleted(): Promise<readonly TaskContinuationSummary[]>;
}

export interface TaskContinuationStorePort extends TaskContinuationQueryPort {
  upsertActive(summary: TaskContinuationSummary): Promise<TaskContinuationSummary>;
  archiveCompleted(summary: TaskContinuationSummary): Promise<TaskContinuationSummary>;
  reopen(taskId: string, nextSummary: TaskContinuationSummary): Promise<TaskContinuationSummary | undefined>;
  /** Remove both active and completed summaries for the task. */
  delete(taskId: string): Promise<void>;
  /** Remove only the completed summary for the task. Tolerates missing entries. */
  deleteCompleted(taskId: string): Promise<void>;
}
