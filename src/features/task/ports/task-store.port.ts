import type {
  Task,
  CreateTaskInput,
  UpdateTaskInput,
  CloseTaskInput,
} from "../domain/task-types.js";

export interface TaskStorePort {
  /** Create a new task with a freshly generated id. Returns the stored task. */
  create(input: CreateTaskInput): Promise<Task>;

  /** Patch an existing task. Throws if id does not exist. */
  update(id: string, patch: UpdateTaskInput): Promise<Task>;

  /** Close a task. Throws if id does not exist. */
  close(id: string, input: CloseTaskInput): Promise<Task>;

  /** Read a single task by id. Returns undefined if not found. */
  get(id: string): Promise<Task | undefined>;

  /** Return all tasks in the store (unordered; callers sort/filter). */
  all(): Promise<readonly Task[]>;
}
