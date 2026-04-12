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
import { assertNoParentCycle, validateTask } from "../domain/task-validators.js";
import { taskAlreadyClosed, taskNotFound, unknownDependency } from "../domain/task-errors.js";
import { MaestroError } from "@/shared/errors.js";
import {
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
  indexTasksById,
} from "../domain/task-types.js";

const MAX_ID_RETRIES = 5;
const LOCK_RETRY_DELAY_MS = 10;
const LOCK_RETRY_COUNT = 100;
const LOCK_STALE_MS = 30_000;

interface TaskStoreLockMetadata {
  readonly pid: number;
  readonly createdAt: string;
}

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
      const byId = indexTasksById(Array.from(tasks.values()));

      if (input.dependsOn && input.dependsOn.length > 0) {
        const missing = input.dependsOn.filter((id) => !byId.has(id));
        if (missing.length > 0) {
          throw unknownDependency("<new task>", missing);
        }
      }

      if (input.parentId !== undefined && !byId.has(input.parentId)) {
        throw taskNotFound(input.parentId);
      }

      let id: string | undefined;
      for (let attempt = 0; attempt < MAX_ID_RETRIES; attempt++) {
        const candidate = generateTaskId();
        if (!tasks.has(candidate)) {
          id = candidate;
          break;
        }
      }
      if (id === undefined) {
        throw new MaestroError(`Failed to generate a unique task id after ${MAX_ID_RETRIES} attempts`, [
          "Retry the command to generate a fresh task id",
          "If the problem persists, inspect .maestro/tasks/tasks.jsonl for duplicate ids",
        ]);
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

      if (patch.parentId !== undefined && patch.parentId !== "") {
        if (!tasks.has(patch.parentId)) {
          throw taskNotFound(patch.parentId);
        }
        assertNoParentCycle(id, patch.parentId, indexTasksById(Array.from(tasks.values())));
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
      if (existing.status === "closed") {
        throw taskAlreadyClosed(id);
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
    const lineById = new Map<string, number>();
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
      const firstLine = lineById.get(validated.id);
      if (firstLine !== undefined) {
        throw new MaestroError(`Task storage contains duplicate id '${validated.id}' at lines ${firstLine} and ${lineNumber}: ${this.tasksPath()}`, [
          "Remove or repair the duplicate task record before retrying",
          "Task mutations are blocked to avoid dropping one of the records",
        ]);
      }
      lineById.set(validated.id, lineNumber);
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
      const metadata = await this.readLockMetadata(lockPath);
      if (metadata && isProcessAlive(metadata.pid)) {
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
          await handle.writeFile(serializeLockMetadata());
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

  private async readLockMetadata(lockPath: string): Promise<TaskStoreLockMetadata | undefined> {
    const raw = await readText(lockPath);
    if (!raw) return undefined;
    try {
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      if (typeof parsed.pid !== "number" || !Number.isInteger(parsed.pid)) {
        return undefined;
      }
      if (typeof parsed.createdAt !== "string") {
        return undefined;
      }
      return {
        pid: parsed.pid,
        createdAt: parsed.createdAt,
      };
    } catch {
      return undefined;
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

function serializeLockMetadata(): string {
  const metadata: TaskStoreLockMetadata = {
    pid: process.pid,
    createdAt: new Date().toISOString(),
  };
  return `${JSON.stringify(metadata)}\n`;
}

function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    const errno = error as NodeJS.ErrnoException;
    if (errno.code === "ESRCH") {
      return false;
    }
    if (errno.code === "EPERM") {
      return true;
    }
    return false;
  }
}
