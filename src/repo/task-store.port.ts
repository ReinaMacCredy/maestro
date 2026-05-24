import type { Task, TaskId } from "../types/task.js";
import type { TaskState } from "../types/task-state.js";

export interface CreateTaskInput {
  readonly slug: string;
  readonly title: string;
  readonly state: TaskState;
  readonly spec_path?: string;
  readonly mission_id?: string;
  readonly parent_id?: TaskId;
  readonly worktree_path?: string;
  readonly blocked_by?: readonly string[];
  readonly id?: TaskId;
}

export type TaskPatch = Partial<
  Omit<Task, "id" | "slug" | "parent_id" | "created_at" | "updated_at">
>;

export interface TaskStorePort {
  create(input: CreateTaskInput): Promise<Task>;
  // Atomic bulk insert: either all rows append in one read-modify-write or none
  // do. Decomposing a mission used to call create() per task, so a crash
  // mid-batch left half a mission on disk.
  createMany(inputs: readonly CreateTaskInput[]): Promise<readonly Task[]>;
  splitTask(input: {
    readonly parentId: TaskId;
    readonly parentPatch: TaskPatch;
    readonly childInputs: readonly CreateTaskInput[];
  }): Promise<{ readonly parent: Task; readonly children: readonly Task[] }>;
  get(id: TaskId): Promise<Task | undefined>;
  update(id: TaskId, patch: TaskPatch): Promise<Task>;
  list(): Promise<readonly Task[]>;
  listByState(state: TaskState): Promise<readonly Task[]>;
  listByMissionId(mission_id: string): Promise<readonly Task[]>;
}

export class TaskNotFoundError extends Error {
  readonly taskId: TaskId;
  constructor(taskId: TaskId) {
    super(`Task ${taskId} not found`);
    this.name = "TaskNotFoundError";
    this.taskId = taskId;
  }
}

export class DuplicateSlugError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Task with slug ${slug} already exists`);
    this.name = "DuplicateSlugError";
    this.slug = slug;
  }
}

export class InvalidTaskIdError extends Error {
  constructor(public readonly id: string) {
    super(`Invalid task id format: ${id}`);
    this.name = "InvalidTaskIdError";
  }
}

export class DuplicateTaskIdError extends Error {
  constructor(public readonly id: string) {
    super(`Task id already exists: ${id}`);
    this.name = "DuplicateTaskIdError";
  }
}
