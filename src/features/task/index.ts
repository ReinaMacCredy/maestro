export type {
  Task,
  TaskStatus,
  TaskType,
  TaskPriority,
  CreateTaskInput,
  UpdateTaskInput,
  CloseTaskInput,
  ClaimTaskInput,
  UnclaimTaskInput,
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
  validateDependencyIds,
  assertNoParentCycle,
  assertNoDependencyCycle,
  isTaskStatus,
  isTaskType,
  isTaskPriority,
} from "./domain/task-validators.js";
export type { TaskCandidate, CandidateSourceType } from "./domain/task-candidate.js";
export { validateTaskCandidate } from "./domain/task-candidate.js";
export { extractKeywords } from "./domain/extract-keywords.js";

export type { TaskStorePort } from "./ports/task-store.port.js";
export type {
  CandidateStorePort,
  CreateCandidateInput,
} from "./ports/candidate-store.port.js";
export { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
export { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";

export { createTask } from "./usecases/create-task.usecase.js";
export { showTask } from "./usecases/show-task.usecase.js";
export { listTasks } from "./usecases/list-tasks.usecase.js";
export { updateTask } from "./usecases/update-task.usecase.js";
export { claimTask } from "./usecases/claim-task.usecase.js";
export { unclaimTask } from "./usecases/unclaim-task.usecase.js";
export {
  addTaskDependencies,
  removeTaskDependencies,
} from "./usecases/manage-task-dependencies.usecase.js";
export { closeTask } from "./usecases/close-task.usecase.js";
export {
  readyTasks,
  type TaskBriefing,
} from "./usecases/ready-tasks.usecase.js";
export { captureTaskCandidate } from "./usecases/capture-task-candidate.usecase.js";
export { matchCandidates, type TaskHint } from "./usecases/match-candidates.usecase.js";

export { registerTaskCommand } from "./commands/task.command.js";
export { buildTaskServices } from "./services.js";
export type { TaskServices } from "./services.js";
