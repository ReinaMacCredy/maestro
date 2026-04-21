import type { TaskContinuationSummary } from "../domain/task-continuation-types.js";

export interface TaskContinuationQueryPort {
  getActive(taskId: string): Promise<TaskContinuationSummary | undefined>;
  getCompleted(taskId: string): Promise<TaskContinuationSummary | undefined>;
  listActive(): Promise<readonly TaskContinuationSummary[]>;
}

export interface TaskContinuationStorePort extends TaskContinuationQueryPort {
  upsertActive(summary: TaskContinuationSummary): Promise<TaskContinuationSummary>;
  archiveCompleted(summary: TaskContinuationSummary): Promise<TaskContinuationSummary>;
  reopen(taskId: string, nextSummary: TaskContinuationSummary): Promise<TaskContinuationSummary | undefined>;
  delete(taskId: string): Promise<void>;
}
