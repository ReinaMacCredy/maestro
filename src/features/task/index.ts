export type {
  Task,
  TaskStatus,
  TaskType,
  TaskPriority,
  CreateTaskInput,
  UpdateTaskInput,
  ClaimTaskInput,
  UnclaimTaskInput,
  TaskMetadataPatch,
  ListTasksFilters,
  ReadyTasksFilters,
} from "./domain/task-types.js";
export type {
  TaskContinuationAgent,
  TaskContinuationSummary,
  TaskContinuationEvent,
} from "./domain/task-continuation-types.js";
export {
  TASK_STATUSES,
  TASK_TYPES,
  TASK_PRIORITIES,
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
} from "./domain/task-types.js";
export {
  validateTaskContinuationAgent,
  validateTaskContinuationSummary,
  validateTaskContinuationEvent,
} from "./domain/task-continuation-types.js";
export { TASK_ID_PATTERN, generateTaskId, isTaskId } from "./domain/task-id.js";
export { getUnresolvedBlockerIds } from "./domain/task-state.js";
export {
  validateTask,
  validateCreateInput,
  validateUpdateInput,
  validateBlockIds,
  assertNoParentCycle,
  assertNoBlockCycle,
  isTaskStatus,
  isTaskType,
  isTaskPriority,
} from "./domain/task-validators.js";
export type { TaskCandidate, CandidateSourceType } from "./domain/task-candidate.js";
export { validateTaskCandidate } from "./domain/task-candidate.js";
export { extractKeywords } from "./domain/extract-keywords.js";

export type { TaskQueryPort, TaskStorePort } from "./ports/task-store.port.js";
export type {
  TaskContinuationQueryPort,
  TaskContinuationStorePort,
} from "./ports/task-continuation-store.port.js";
export type { TaskContinuationHistoryPort } from "./ports/task-continuation-history.port.js";
export type {
  CandidateStorePort,
  CreateCandidateInput,
} from "./ports/candidate-store.port.js";
export { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
export { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";
export { FsTaskContinuationStoreAdapter } from "./adapters/fs-task-continuation-store.adapter.js";
export { FsTaskContinuationHistoryStoreAdapter } from "./adapters/fs-task-continuation-history-store.adapter.js";
export type { Contract } from "./domain/contract/contract-types.js";

export { createTask } from "./usecases/create-task.usecase.js";
export { showTask } from "./usecases/show-task.usecase.js";
export { listTasks } from "./usecases/list-tasks.usecase.js";
export { updateTask } from "./usecases/update-task.usecase.js";
export { claimTask } from "./usecases/claim-task.usecase.js";
export { unclaimTask } from "./usecases/unclaim-task.usecase.js";
export { syncTaskMetadata } from "./usecases/sync-task-metadata.usecase.js";
export {
  blockTasks,
  unblockTasks,
} from "./usecases/manage-task-blockers.usecase.js";
export { releaseOwnedTasks } from "./usecases/release-owned-tasks.usecase.js";
export {
  readyTasks,
  type TaskBriefing,
} from "./usecases/ready-tasks.usecase.js";
export { captureTaskCandidate } from "./usecases/capture-task-candidate.usecase.js";
export { matchCandidates, type TaskHint } from "./usecases/match-candidates.usecase.js";
export { planTasks } from "./usecases/plan-tasks.usecase.js";
export {
  buildTaskShowView,
  buildTaskContinuationSummary,
  buildTaskOwnerId,
  deriveAgentFromAssignee,
  loadTaskContinuationSummary,
  parseTaskOwnerId,
  syncTaskContinuation,
} from "./usecases/task-continuation.usecase.js";
export type {
  TaskShowView,
  TaskContinuationDeps,
  ContinuationSummaryOverrides,
  SyncTaskContinuationInput,
} from "./usecases/task-continuation.usecase.js";
export type {
  BatchTaskInput,
  BatchInput,
  BatchCreatedTask,
  BatchResult,
  CreateBatchInput,
} from "./domain/task-batch-types.js";

export { registerTaskCommand } from "./commands/task.command.js";
export { buildTaskServices } from "./services.js";
export type { TaskServices } from "./services.js";
export { transferContractOwnership } from "./usecases/contract/transfer-ownership.usecase.js";
