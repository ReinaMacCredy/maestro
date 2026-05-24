import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { ensureDir, writeText } from "@/shared/lib/fs.js";
import { slugTooLong } from "@/shared/domain/task/domain/task-errors.js";
import { SLUG_MAX_LENGTH } from "@/shared/domain/task/domain/task-slug.js";
import type { Task, TaskId } from "../types/task.js";
import { generateTaskId, TASK_ID_PATTERN } from "../types/task.js";
import { isTaskState, type TaskState } from "../types/task-state.js";
import {
  DuplicateSlugError,
  DuplicateTaskIdError,
  InvalidTaskIdError,
  TaskNotFoundError,
  type CreateTaskInput,
  type TaskPatch,
  type TaskStorePort,
} from "./task-store.port.js";

const DEFAULT_PATH = ".maestro/tasks/tasks.jsonl";

export interface JsonlTaskStoreOptions {
  readonly repoRoot: string;
  readonly file?: string;
  readonly clock?: () => Date;
  readonly idFactory?: () => TaskId;
}

/**
 * Single-writer JSONL adapter. `#queue` is an in-process Promise chain — it
 * serializes writes within ONE Node/Bun process only. Two `maestro` processes
 * mutating the same `.maestro/tasks/tasks.jsonl` concurrently will race; the
 * atomic `tmp+rename` pattern in `writeText` keeps individual writes
 * crash-safe, but the read-modify-write cycle is NOT protected by any
 * file-system lock.
 *
 * This is intentional: the agent harness's expected concurrency model is one
 * driving CLI process per task. If that assumption changes (multi-process
 * driver, IDE plugin co-driving an MCP server, etc.), this adapter needs a
 * file-lock-based serializer — `#queue` is not enough.
 */
export class JsonlTaskStore implements TaskStorePort {
  readonly #file: string;
  readonly #clock: () => Date;
  readonly #idFactory: () => TaskId;
  // In-process write serializer. See class-level doc above for the
  // cross-process caveat — this Promise chain only orders writes within ONE
  // process; concurrent processes still race.
  #queue: Promise<void> = Promise.resolve();

  constructor(options: JsonlTaskStoreOptions) {
    this.#file = join(options.repoRoot, options.file ?? DEFAULT_PATH);
    this.#clock = options.clock ?? (() => new Date());
    this.#idFactory = options.idFactory ?? generateTaskId;
  }

  #assertValidProvidedId(id: TaskId): void {
    if (!TASK_ID_PATTERN.test(id)) {
      throw new InvalidTaskIdError(id);
    }
  }

  async create(input: CreateTaskInput): Promise<Task> {
    return this.#mutate(async (tasks) => {
      if (input.id !== undefined) {
        this.#assertValidProvidedId(input.id);
        if (tasks.some((t) => t.id === input.id)) {
          throw new DuplicateTaskIdError(input.id);
        }
      }
      if (tasks.some((t) => t.slug === input.slug)) {
        throw new DuplicateSlugError(input.slug);
      }
      const now = this.#clock().toISOString();
      const task: Task = {
        id: input.id ?? this.#idFactory(),
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        mission_id: input.mission_id,
        blocked_by: input.blocked_by ?? [],
        ...(input.parent_id !== undefined ? { parent_id: input.parent_id } : {}),
        ...(input.worktree_path !== undefined ? { worktree_path: input.worktree_path } : {}),
        created_at: now,
        updated_at: now,
      };
      tasks.push(task);
      return task;
    });
  }

  async createMany(inputs: readonly CreateTaskInput[]): Promise<readonly Task[]> {
    if (inputs.length === 0) return [];
    return this.#mutate(async (tasks): Promise<readonly Task[]> => {
      const inBatch = new Set<string>();
      const inBatchIds = new Set<TaskId>();
      const existing = new Set(tasks.map((t) => t.slug));
      const existingIds = new Set(tasks.map((t) => t.id));
      for (const input of inputs) {
        if (input.id !== undefined) {
          this.#assertValidProvidedId(input.id);
          if (existingIds.has(input.id) || inBatchIds.has(input.id)) {
            throw new DuplicateTaskIdError(input.id);
          }
          inBatchIds.add(input.id);
        }
        if (inBatch.has(input.slug) || existing.has(input.slug)) {
          throw new DuplicateSlugError(input.slug);
        }
        inBatch.add(input.slug);
      }
      const now = this.#clock().toISOString();
      const created: Task[] = [];
      for (const input of inputs) {
        const task: Task = {
          id: input.id ?? this.#idFactory(),
          slug: input.slug,
          title: input.title,
          state: input.state,
          spec_path: input.spec_path,
          mission_id: input.mission_id,
          blocked_by: input.blocked_by ?? [],
          ...(input.parent_id !== undefined ? { parent_id: input.parent_id } : {}),
          ...(input.worktree_path !== undefined ? { worktree_path: input.worktree_path } : {}),
          created_at: now,
          updated_at: now,
        };
        tasks.push(task);
        created.push(task);
      }
      return created;
    });
  }

  async splitTask(input: {
    readonly parentId: TaskId;
    readonly parentPatch: TaskPatch;
    readonly childInputs: readonly CreateTaskInput[];
  }): Promise<{ readonly parent: Task; readonly children: readonly Task[] }> {
    if (input.childInputs.length === 0) {
      throw new Error("splitTask requires at least one child");
    }
    return this.#mutate(async (tasks): Promise<{ readonly parent: Task; readonly children: readonly Task[] }> => {
      const parentIdx = tasks.findIndex((t) => t.id === input.parentId);
      if (parentIdx === -1) throw new TaskNotFoundError(input.parentId);

      const inBatchSlugs = new Set<string>();
      const inBatchIds = new Set<TaskId>();
      const existingSlugs = new Set(tasks.map((t) => t.slug));
      const existingIds = new Set(tasks.map((t) => t.id));
      for (const ci of input.childInputs) {
        if (ci.id !== undefined) {
          this.#assertValidProvidedId(ci.id);
          if (existingIds.has(ci.id) || inBatchIds.has(ci.id)) {
            throw new DuplicateTaskIdError(ci.id);
          }
          inBatchIds.add(ci.id);
        }
        if (ci.slug.length > SLUG_MAX_LENGTH) {
          throw slugTooLong(ci.slug, SLUG_MAX_LENGTH);
        }
        if (inBatchSlugs.has(ci.slug) || existingSlugs.has(ci.slug)) {
          throw new DuplicateSlugError(ci.slug);
        }
        inBatchSlugs.add(ci.slug);
      }

      const now = this.#clock().toISOString();
      const parent = tasks[parentIdx]!;
      const updatedParent: Task = { ...parent, ...input.parentPatch, updated_at: now };
      tasks[parentIdx] = updatedParent;

      const children: Task[] = input.childInputs.map((ci) => ({
        id: ci.id ?? this.#idFactory(),
        slug: ci.slug,
        title: ci.title,
        state: ci.state,
        ...(ci.spec_path !== undefined ? { spec_path: ci.spec_path } : {}),
        ...(ci.mission_id !== undefined ? { mission_id: ci.mission_id } : {}),
        ...(ci.worktree_path !== undefined ? { worktree_path: ci.worktree_path } : {}),
        blocked_by: ci.blocked_by ?? [],
        parent_id: input.parentId,
        created_at: now,
        updated_at: now,
      }));
      tasks.push(...children);
      return { parent: updatedParent, children };
    });
  }

  async get(id: TaskId): Promise<Task | undefined> {
    const tasks = await this.#read();
    return tasks.find((t) => t.id === id);
  }

  async update(id: TaskId, patch: TaskPatch): Promise<Task> {
    return this.#mutate(async (tasks) => {
      const idx = tasks.findIndex((t) => t.id === id);
      if (idx === -1) throw new TaskNotFoundError(id);
      const existing = tasks[idx];
      if (!existing) throw new TaskNotFoundError(id);
      const next: Task = {
        ...existing,
        ...patch,
        updated_at: this.#clock().toISOString(),
      };
      tasks[idx] = next;
      return next;
    });
  }

  async list(): Promise<readonly Task[]> {
    return this.#read();
  }

  async listByState(state: TaskState): Promise<readonly Task[]> {
    const tasks = await this.#read();
    return tasks.filter((t) => t.state === state);
  }

  async listByMissionId(mission_id: string): Promise<readonly Task[]> {
    const tasks = await this.#read();
    return tasks.filter((t) => t.mission_id === mission_id);
  }

  #mutate<T>(fn: (tasks: Task[]) => Promise<T>): Promise<T> {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason: unknown) => void;
    const result = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    this.#queue = this.#queue.then(async () => {
      try {
        const tasks = [...(await this.#read())];
        const r = await fn(tasks);
        await this.#write(tasks);
        resolve(r);
      } catch (e) {
        reject(e);
      }
    });
    return result;
  }

  async #read(): Promise<readonly Task[]> {
    let text: string;
    try {
      text = await readFile(this.#file, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    const out: Task[] = [];
    let lineNo = 0;
    for (const line of text.split("\n")) {
      lineNo += 1;
      if (line.length === 0) continue;
      const row = JSON.parse(line) as Record<string, unknown>;
      // Validate-on-read so malformed rows surface as explicit errors instead
      // of silently corrupting filters that key on mission_id.
      if (typeof row.id !== "string" || typeof row.slug !== "string" || typeof row.title !== "string") {
        throw new Error(`${this.#file}:${lineNo}: task row missing required string fields`);
      }
      if (typeof row.state !== "string" || !isTaskState(row.state)) {
        throw new Error(
          `${this.#file}:${lineNo}: task row has unknown state '${String(row.state)}'; edit the file manually or remove it`,
        );
      }
      if ("plan_id" in row) {
        throw new Error(
          `${this.#file}:${lineNo}: task row carries legacy 'plan_id' field; rename it to 'mission_id' manually or remove the row`,
        );
      }
      out.push(row as unknown as Task);
    }
    return out;
  }

  async #write(tasks: readonly Task[]): Promise<void> {
    await ensureDir(dirname(this.#file));
    const body = tasks.map((t) => JSON.stringify(t)).join("\n");
    // writeText is tmp-file + rename, so a crash mid-write can't leave a
    // half-truncated jsonl that fails validate-on-read on the next boot.
    await writeText(this.#file, body.length > 0 ? `${body}\n` : "");
  }
}
