import type { TaskStorePort } from "./ports/task-store.port.js";
import { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";

export interface TaskServices {
  readonly taskStore: TaskStorePort;
}

export function buildTaskServices(projectDir: string): TaskServices {
  return {
    taskStore: new JsonlTaskStoreAdapter(projectDir),
  };
}
