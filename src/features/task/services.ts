import type { TaskStorePort } from "./ports/task-store.port.js";
import type { CandidateStorePort } from "./ports/candidate-store.port.js";
import { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
import { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";

export interface TaskServices {
  readonly taskStore: TaskStorePort;
  readonly taskCandidateStore: CandidateStorePort;
}

export function buildTaskServices(projectDir: string): TaskServices {
  return {
    taskStore: new JsonlTaskStoreAdapter(projectDir),
    taskCandidateStore: new FsCandidateStoreAdapter(projectDir),
  };
}
