/**
 * Legacy v1 Task domain -- stable shared location.
 *
 * Surviving consumers (handoff, gc, verdict, tui, shared/lib/projection,
 * services.ts) all import from here. Moved from src/features/task/ so that
 * Phase 5 (D-task) can delete the v1 CLI verbs without breaking those
 * consumers.
 *
 * Confirmed v2 collisions requiring Legacy* rename:
 *   - Task        → LegacyTask        (src/types/task.ts has Task)
 *   - TaskStorePort → LegacyTaskStorePort (src/repo/task-store.port.ts has TaskStorePort)
 *
 * All other names are unchanged (no v2 namespace collision).
 *
 * MCP is NOT rewired here — MCP rewire is D-task-MCP (see shim at
 * src/features/task/index.ts which re-exports from this module).
 */

// ---- Domain types ----
export type {
  Task as LegacyTask,
  TaskStatus,
  TaskType,
  TaskPriority,
  TaskSummary,
  CreateTaskInput,
  UpdateTaskInput,
  ClaimTaskInput,
  UnclaimTaskInput,
  TaskMetadataPatch,
  ListTasksFilters,
  ReadyTasksFilters,
  TaskWorkType,
  TaskReceipt,
  BuildTaskReceiptInput,
  TaskMutationInput,
  UpdateTaskResult,
} from "./domain/task-types.js";
export {
  TASK_STATUSES,
  TASK_TYPES,
  TASK_PRIORITIES,
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
  indexTasksById,
  buildTaskReceipt,
} from "./domain/task-types.js";

export type {
  TaskContinuationAgent,
  TaskContinuationSummary,
  TaskContinuationEvent,
} from "./domain/task-continuation-types.js";
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

export type {
  BatchTaskInput,
  BatchInput,
  BatchCreatedTask,
  BatchResult,
  CreateBatchInput,
} from "./domain/task-batch-types.js";

export { generateContractAmendmentId } from "./domain/contract/contract-state.js";

// ---- Ports ----
export type {
  TaskQueryPort,
  TaskStorePort as LegacyTaskStorePort,
} from "./ports/task-store.port.js";
export type { TaskContinuationStorePort } from "./ports/task-continuation-store.port.js";
export type { TaskContinuationHistoryPort } from "./ports/task-continuation-history.port.js";
export type {
  CandidateStorePort,
  CreateCandidateInput,
} from "./ports/candidate-store.port.js";
export type {
  ContractStorePort,
  ContractStoreQueryPort,
} from "./ports/contract-store.port.js";
export type { ContractVersionStorePort } from "./ports/contract-version-store.port.js";
export type { GitAnchorPort } from "./ports/git-anchor.port.js";
export type { RunStateStorePort, RunStateDelta } from "./ports/run-state-store.port.js";

// ---- Adapters ----
export { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
export { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";
export { FsTaskContinuationStoreAdapter } from "./adapters/fs-task-continuation-store.adapter.js";
export { FsTaskContinuationHistoryStoreAdapter } from "./adapters/fs-task-continuation-history-store.adapter.js";
export { FsRunStateStoreAdapter } from "./adapters/fs-run-state-store.adapter.js";
export { FsContractStoreAdapter } from "./adapters/fs-contract-store.adapter.js";
export { FsContractVersionStoreAdapter } from "./adapters/fs-contract-version-store.adapter.js";
export { ShellGitAnchorAdapter } from "./adapters/git-anchor.adapter.js";

// ---- Usecases ----
export { createTask } from "./usecases/create-task.usecase.js";
export { listTasks } from "./usecases/list-tasks.usecase.js";
export { updateTask } from "./usecases/update-task.usecase.js";
export { claimTask } from "./usecases/claim-task.usecase.js";
export { unclaimTask } from "./usecases/unclaim-task.usecase.js";
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
export { buildBatchInputSchema } from "./usecases/batch-input-schema.usecase.js";
export { nextTask } from "./usecases/next-task.usecase.js";
export type {
  NextTaskInput,
  NextTaskResult,
  NextTaskReason,
} from "./usecases/next-task.usecase.js";
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
export { buildContractWorkflows } from "./usecases/contract-workflows.usecase.js";
export type {
  ContractAmendmentCommand,
  ContractCriterionDraftInput,
  ContractVerdictInput,
  ContractWorkflows,
  CreateContractInput,
  EditContractInput,
  ListContractsFilters,
  LockContractInput,
} from "./usecases/contract-workflows.usecase.js";
export { proposeContract } from "./usecases/propose-contract.usecase.js";
export { approveContract } from "./usecases/approve-contract.usecase.js";
export { amendContract } from "./usecases/amend-contract.usecase.js";
export type { AmendContractInput } from "./usecases/amend-contract.usecase.js";
export { getCurrentContract } from "./usecases/get-current-contract.usecase.js";
export { getContractHistory } from "./usecases/get-contract-history.usecase.js";
export { showTask } from "./usecases/show-task.usecase.js";
export { inspectTask } from "./usecases/inspect-task.usecase.js";
export type {
  TaskInspectionDeps,
  TaskInspectionView,
} from "./usecases/inspect-task.usecase.js";
export {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
} from "./usecases/compose-task-introspection.usecase.js";
export type {
  TaskIntrospectionDeps,
  TaskIntrospectionView,
} from "./usecases/compose-task-introspection.usecase.js";

// ---- Services ----
export { buildTaskServices } from "./services.js";
export type { TaskServices } from "./services.js";
