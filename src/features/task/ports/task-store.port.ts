import type {
  Task,
  CreateTaskInput,
  TaskMutationInput,
  UpdateTaskInput,
  UpdateTaskResult,
} from "../domain/task-types.js";

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
}
