import type {
  Task,
  CreateTaskInput,
  TaskMetadataPatch,
  TaskMutationInput,
  UpdateTaskInput,
  UpdateTaskResult,
} from "../domain/task-types.js";
import type { BatchResult, CreateBatchInput } from "../domain/task-batch-types.js";

export interface TaskQueryPort {
  /** Read a single task by id. Returns undefined if not found. */
  get(id: string): Promise<Task | undefined>;

  /** Return all tasks in the store (unordered; callers sort/filter). */
  all(): Promise<readonly Task[]>;
}

export interface TaskStorePort extends TaskQueryPort {
  /** Create a new task with a freshly generated id. Returns the stored task. */
  create(input: CreateTaskInput): Promise<Task>;

  /**
   * Create N tasks atomically under a single lock.
   *
   * References inside each input (`parentRef`, `blockedByRefs`) are already
   * resolved: a number is a zero-based index into the batch array, a string
   * is a pre-validated real task id. The adapter generates ids for the batch
   * members and substitutes the numeric refs just before write.
   *
   * When `receipt` is passed, the adapter writes a batch receipt file
   * (`.maestro/tasks/batches/<batchId>.json`) inside the same lock, BEFORE
   * the tasks.jsonl write. If the process dies between the two writes, the
   * receipt points to ids that will not exist, and the replay path fails
   * the drift check instead of silently double-creating the batch.
   *
   * Rejects the whole batch on any cycle, unknown real-id reference, or id
   * generation failure. No partial writes.
   */
  createBatch(
    inputs: readonly CreateBatchInput[],
    receipt?: { readonly batchId: string; readonly names: readonly (string | undefined)[] },
  ): Promise<readonly Task[]>;

  /**
   * Patch an existing task. Throws if id does not exist.
   *
   * `autoClaimed` is carried on the result rather than inferred from the task
   * because callers lack the pre-update snapshot needed to tell an explicit
   * claim-then-update from an auto-claim folded into the same write.
   */
  update(id: string, patch: UpdateTaskInput, opts?: TaskMutationInput): Promise<UpdateTaskResult>;

  /** Claim an existing task for a session, optionally forcing takeover. */
  claim(id: string, sessionId: string, opts?: { force?: boolean; checkBusy?: boolean }): Promise<Task>;

  /** Release task ownership for a session, optionally forcing release. */
  unclaim(id: string, sessionId: string, opts?: { force?: boolean }): Promise<Task>;

  /** Add blocker edges to an existing task. */
  block(id: string, blockedTaskIds: readonly string[], opts?: TaskMutationInput): Promise<Task>;

  /** Remove blocker edges from an existing task. */
  unblock(id: string, blockedTaskIds: readonly string[], opts?: TaskMutationInput): Promise<Task>;

  /** Release unresolved tasks owned by a session back to the pending queue. */
  releaseOwned(sessionId: string): Promise<readonly Task[]>;

  /** Reopen a completed task back into the pending queue. */
  reopen(id: string): Promise<Task>;

  /** Delete a task and remove its graph references from the remaining store. */
  delete(id: string): Promise<Task>;

  /**
   * Bump `lastActivityAt` on a claimed task without any other state change.
   * Used by `task heartbeat` so long-running sessions signal they are alive.
   * Throws if the caller is not the current owner (unless forced).
   */
  heartbeat(id: string, sessionId: string, opts?: { force?: boolean }): Promise<Task>;

  /**
   * Look up a stored batch receipt for idempotency replay.
   *
   * Returns undefined if no prior batch submitted the given id. Does not
   * validate whether cached ids still exist in the store -- callers handle
   * drift detection.
   */
  findBatchReceipt(batchId: string): Promise<BatchResult | undefined>;

  /** Persist internal task metadata without widening the public task update surface. */
  syncMetadata(id: string, patch: TaskMetadataPatch): Promise<Task>;

}
