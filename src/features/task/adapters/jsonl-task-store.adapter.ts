/**
 * JSONL-backed task store.
 * Storage layout: `.maestro/tasks/tasks.jsonl` (one JSON object per line).
 *
 * Read: load whole file, parse each non-empty line with validator.
 * Write: serialize all tasks, write atomically via writeText (routes through
 *        writeAtomic which does a temp-file + rename).
 *
 * Concurrency: last-writer-wins via atomic rename. No torn writes, but no
 *              mutex. Round-one scope; upgrade when a real race appears.
 */

import { join } from "node:path";
import type { Task, CreateTaskInput, UpdateTaskInput, CloseTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, writeText } from "@/shared/lib/fs.js";
import { generateTaskId } from "../domain/task-id.js";
import { validateTask } from "../domain/task-validators.js";
import { taskNotFound } from "../domain/task-errors.js";
import {
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
} from "../domain/task-types.js";

const MAX_ID_RETRIES = 5;

export class JsonlTaskStoreAdapter implements TaskStorePort {
  constructor(private readonly baseDir: string) {}

  private tasksDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks");
  }

  private tasksPath(): string {
    return join(this.tasksDir(), "tasks.jsonl");
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
  }

  async update(id: string, patch: UpdateTaskInput): Promise<Task> {
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
  }

  async close(id: string, input: CloseTaskInput): Promise<Task> {
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
  }

  // ============================
  // Internal helpers
  // ============================

  private async readAll(): Promise<Map<string, Task>> {
    const raw = await readText(this.tasksPath());
    if (raw === undefined) return new Map();

    const result = new Map<string, Task>();
    const lines = raw.split("\n");
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      let parsed: unknown;
      try {
        parsed = JSON.parse(trimmed);
      } catch {
        // Skip malformed lines rather than aborting the whole read.
        // This matches the tolerance of the mission adapter's validator filter.
        continue;
      }
      const validated = validateTask(parsed);
      if (validated) {
        result.set(validated.id, validated);
      }
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
