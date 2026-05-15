import type { TaskContinuationEvent } from "../domain/task-continuation-types.js";

export interface TaskContinuationHistoryPort {
  append(taskId: string, event: TaskContinuationEvent): Promise<void>;
  listRecent(taskId: string, limit: number): Promise<readonly TaskContinuationEvent[]>;
  delete(taskId: string): Promise<void>;
}
