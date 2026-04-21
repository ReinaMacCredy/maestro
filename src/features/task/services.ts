import type { TaskStorePort } from "./ports/task-store.port.js";
import type { CandidateStorePort } from "./ports/candidate-store.port.js";
import type { TaskContinuationStorePort } from "./ports/task-continuation-store.port.js";
import type { TaskContinuationHistoryPort } from "./ports/task-continuation-history.port.js";
import { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
import { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "./adapters/fs-task-continuation-store.adapter.js";
import { FsTaskContinuationHistoryStoreAdapter } from "./adapters/fs-task-continuation-history-store.adapter.js";

export interface TaskServices {
  readonly taskStore: TaskStorePort;
  readonly taskCandidateStore: CandidateStorePort;
  readonly taskContinuationStore: TaskContinuationStorePort;
  readonly taskContinuationHistory: TaskContinuationHistoryPort;
}

export function buildTaskServices(projectDir: string): TaskServices {
  return {
    taskStore: new JsonlTaskStoreAdapter(projectDir),
    taskCandidateStore: new FsCandidateStoreAdapter(projectDir),
    taskContinuationStore: new FsTaskContinuationStoreAdapter(projectDir),
    taskContinuationHistory: new FsTaskContinuationHistoryStoreAdapter(projectDir),
  };
}
