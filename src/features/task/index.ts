export type {
  Task,
  TaskStatus,
  TaskType,
  TaskPriority,
  CreateTaskInput,
  UpdateTaskInput,
  CloseTaskInput,
  ListTasksFilters,
  ReadyTasksFilters,
} from "./domain/task-types.js";
export {
  TASK_STATUSES,
  TASK_TYPES,
  TASK_PRIORITIES,
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
} from "./domain/task-types.js";
export { TASK_ID_PATTERN, generateTaskId, isTaskId } from "./domain/task-id.js";
export {
  validateTask,
  validateCreateInput,
  validateUpdateInput,
  assertNoParentCycle,
  isTaskStatus,
  isTaskType,
  isTaskPriority,
} from "./domain/task-validators.js";

export type { TaskStorePort } from "./ports/task-store.port.js";
export { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";

export { registerTaskCommand } from "./commands/task.command.js";
export { buildTaskServices } from "./services.js";
export type { TaskServices } from "./services.js";
