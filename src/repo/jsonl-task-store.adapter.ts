import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Task, TaskId } from "../types/task.js";
import { generateTaskId } from "../types/task.js";
import { isTaskState, type TaskState } from "../types/task-state.js";
import {
  DuplicateSlugError,
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

export class JsonlTaskStore implements TaskStorePort {
  readonly #file: string;
  readonly #clock: () => Date;
  readonly #idFactory: () => TaskId;
  #queue: Promise<void> = Promise.resolve();

  constructor(options: JsonlTaskStoreOptions) {
    this.#file = join(options.repoRoot, options.file ?? DEFAULT_PATH);
    this.#clock = options.clock ?? (() => new Date());
    this.#idFactory = options.idFactory ?? generateTaskId;
  }

  async create(input: CreateTaskInput): Promise<Task> {
    return this.#mutate(async (tasks) => {
      if (tasks.some((t) => t.slug === input.slug)) {
        throw new DuplicateSlugError(input.slug);
      }
      const now = this.#clock().toISOString();
      const task: Task = {
        id: this.#idFactory(),
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        mission_id: input.mission_id,
        blocked_by: input.blocked_by ?? [],
        created_at: now,
        updated_at: now,
      };
      tasks.push(task);
      return task;
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
      // Validate-on-read so legacy v1 rows (carrying `plan_id`) surface as
      // explicit errors instead of silently corrupting filters that key on
      // mission_id. Run `maestro setup` to rewrite legacy task rows.
      if (typeof row.id !== "string" || typeof row.slug !== "string" || typeof row.title !== "string") {
        throw new Error(`${this.#file}:${lineNo}: task row missing required string fields`);
      }
      if (typeof row.state !== "string" || !isTaskState(row.state)) {
        throw new Error(
          `${this.#file}:${lineNo}: task row has unknown state '${String(row.state)}' (run \`maestro setup\` to migrate)`,
        );
      }
      if ("plan_id" in row) {
        throw new Error(
          `${this.#file}:${lineNo}: task row carries legacy 'plan_id' field; run \`maestro setup\` to migrate it to 'mission_id'`,
        );
      }
      out.push(row as unknown as Task);
    }
    return out;
  }

  async #write(tasks: readonly Task[]): Promise<void> {
    await mkdir(dirname(this.#file), { recursive: true });
    const body = tasks.map((t) => JSON.stringify(t)).join("\n");
    await writeFile(this.#file, body.length > 0 ? `${body}\n` : "", "utf8");
  }
}
