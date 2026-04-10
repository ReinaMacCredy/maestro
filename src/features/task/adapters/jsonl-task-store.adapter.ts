/**
 * JSONL-backed task store.
 * Storage layout: `.maestro/tasks/tasks.jsonl` (one JSON object per line).
 *
 * Read: load whole file, parse each non-empty line with validator.
 * Write: serialize all tasks, write atomically via writeText (routes through
 *        writeAtomic which does a temp-file + rename).
 *
 * Concurrency: mutation commands take a lock around the full read/modify/write
 * cycle so claims and updates do not clobber each other.
 */

import { join } from "node:path";
import { open, stat } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import type { Task, CreateTaskInput, UpdateTaskInput, CloseTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, removeIfExists, writeText } from "@/shared/lib/fs.js";
import { generateTaskId } from "../domain/task-id.js";
import { validateTask } from "../domain/task-validators.js";
import { taskNotFound } from "../domain/task-errors.js";
import { MaestroError } from "@/shared/errors.js";
import {
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
} from "../domain/task-types.js";

const MAX_ID_RETRIES = 5;
const LOCK_RETRY_DELAY_MS = 10;
const LOCK_RETRY_COUNT = 100;
const LOCK_STALE_MS = 30_000;

export class JsonlTaskStoreAdapter implements TaskStorePort {
  constructor(private readonly baseDir: string) {}

  private tasksDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks");
  }

  private tasksPath(): string {
    return join(this.tasksDir(), "tasks.jsonl");
  }

  private lockPath(): string {
    return join(this.tasksDir(), ".tasks.lock");
  }

  async all(): Promise<readonly Task[]> {
    const tasks = await this.readAll();
    return Array.from(tasks.values());
  }

  async get(id: string): Promise<Task | undefined> {
    const tasks = await this.readAll();
    return tasks.get(id);
  }

  async create(input: CreateTaskInput): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();

      let id: string | undefined;
      for (let attempt = 0; attempt < MAX_ID_RETRIES; attempt++) {
        const candidate = generateTaskId();
        if (!tasks.has(candidate)) {
          id = candidate;
          break;
        }
      }
      if (id === undefined) {
        throw new Error(`Failed to generate a unique task id after ${MAX_ID_RETRIES} attempts`);
      }

      const now = new Date().toISOString();
      const task: Task = {
        id,
        title: input.title,
        description: input.description,
        type: input.type ?? DEFAULT_TASK_TYPE,
        priority: input.priority ?? DEFAULT_TASK_PRIORITY,
        status: DEFAULT_TASK_STATUS,
        parentId: input.parentId,
        labels: input.labels ?? [],
        dependsOn: input.dependsOn ?? [],
        assignee: input.assignee,
        createdAt: now,
        updatedAt: now,
      };

      tasks.set(id, task);
      await this.writeAll(tasks);
      return task;
    });
  }

  async update(id: string, patch: UpdateTaskInput): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const existing = tasks.get(id);
      if (!existing) {
        throw taskNotFound(id);
      }

      const labels = applyLabelPatch(existing.labels, patch.addLabels, patch.removeLabels);

      const updated: Task = {
        ...existing,
        title: patch.title ?? existing.title,
        description: patch.description !== undefined ? patch.description : existing.description,
        type: patch.type ?? existing.type,
        priority: patch.priority ?? existing.priority,
        status: patch.status ?? existing.status,
        parentId: patch.parentId === "" ? undefined : (patch.parentId ?? existing.parentId),
        assignee: patch.assignee !== undefined
          ? (patch.assignee === "" ? undefined : patch.assignee)
          : existing.assignee,
        labels,
        deferUntil: patch.deferUntil !== undefined
          ? (patch.deferUntil === "" ? undefined : patch.deferUntil)
          : existing.deferUntil,
        updatedAt: new Date().toISOString(),
      };

      tasks.set(id, updated);
      await this.writeAll(tasks);
      return updated;
    });
  }

  async close(id: string, input: CloseTaskInput): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const existing = tasks.get(id);
      if (!existing) {
        throw taskNotFound(id);
      }

      const closed: Task = {
        ...existing,
        status: "closed",
        closeReason: input.reason,
        updatedAt: new Date().toISOString(),
      };

      tasks.set(id, closed);
      await this.writeAll(tasks);
      return closed;
    });
  }

  // ============================
  // Internal helpers
  // ============================

  private async readAll(): Promise<Map<string, Task>> {
    const raw = await readText(this.tasksPath());
    if (raw === undefined) return new Map();

    const result = new Map<string, Task>();
    const lines = raw.split("\n");
    for (const [index, line] of lines.entries()) {
      const lineNumber = index + 1;
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      let parsed: unknown;
      try {
        parsed = JSON.parse(trimmed);
      } catch {
        throw new MaestroError(`Task storage is corrupted at line ${lineNumber}: ${this.tasksPath()}`, [
          "Fix or remove the malformed JSON line before retrying",
          "Task mutations are blocked to avoid dropping persisted data",
        ]);
      }
      const validated = validateTask(parsed);
      if (!validated) {
        throw new MaestroError(`Task storage contains an invalid record at line ${lineNumber}: ${this.tasksPath()}`, [
          "Repair the invalid task JSON before retrying",
          "Task mutations are blocked to avoid rewriting incomplete data",
        ]);
      }
      result.set(validated.id, validated);
    }
    return result;
  }

  private async writeAll(tasks: ReadonlyMap<string, Task>): Promise<void> {
    await ensureDir(this.tasksDir());
    const lines: string[] = [];
    for (const task of tasks.values()) {
      lines.push(JSON.stringify(task));
    }
    const content = lines.length === 0 ? "" : lines.join("\n") + "\n";
    await writeText(this.tasksPath(), content);
  }

  private async removeStaleLock(lockPath: string): Promise<boolean> {
    try {
      const lockStat = await stat(lockPath);
      if (Date.now() - lockStat.mtimeMs < LOCK_STALE_MS) {
        return false;
      }
      await removeIfExists(lockPath);
      return true;
    } catch (error) {
      const errno = error as NodeJS.ErrnoException;
      if (errno.code === "ENOENT") {
        return false;
      }
      throw error;
    }
  }

  private async withLock<T>(fn: () => Promise<T>): Promise<T> {
    await ensureDir(this.tasksDir());
    const lockPath = this.lockPath();
    let attempt = 0;

    while (true) {
      try {
        const handle = await open(lockPath, "wx");
        try {
          return await fn();
        } finally {
          await handle.close();
          await removeIfExists(lockPath);
        }
      } catch (error) {
        const errno = error as NodeJS.ErrnoException;
        if (errno.code !== "EEXIST") {
          throw error;
        }
        if (await this.removeStaleLock(lockPath)) {
          continue;
        }
        if (attempt >= LOCK_RETRY_COUNT) {
          throw new MaestroError(`Task store lock is still active: ${lockPath}`, [
            "Retry once the other task command finishes",
            `If this lock is stale, remove it manually: rm ${lockPath}`,
          ]);
        }
        attempt += 1;
        await sleep(LOCK_RETRY_DELAY_MS);
      }
    }
  }
}

/** Apply --add-label / --remove-label patches while preserving order + dedup. */
function applyLabelPatch(
  current: readonly string[],
  add: readonly string[] | undefined,
  remove: readonly string[] | undefined,
): readonly string[] {
  if (!add && !remove) return current;

  const removeSet = new Set(remove ?? []);
  const result: string[] = current.filter((label) => !removeSet.has(label));

  if (add) {
    const existing = new Set(result);
    for (const label of add) {
      if (!existing.has(label)) {
        result.push(label);
        existing.add(label);
      }
    }
  }

  return result;
}
