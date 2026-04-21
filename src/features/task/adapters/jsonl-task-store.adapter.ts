/**
 * JSONL-backed task store.
 * Storage layout: `.maestro/tasks/tasks.jsonl` (one JSON object per line).
 *
 * Read: load whole file, parse each non-empty line with validator.
 * Write: serialize all tasks, write atomically via writeText.
 *
 * Concurrency: mutation commands take a lock around the full read/modify/write
 * cycle so claims and updates do not clobber each other.
 */

import { join } from "node:path";
import { open, stat } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import type {
  Task,
  CreateTaskInput,
  TaskMutationInput,
  TaskReceipt,
  UpdateTaskInput,
  UpdateTaskResult,
} from "../domain/task-types.js";
import type { BatchResult, CreateBatchInput } from "../domain/task-batch-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, removeIfExists, writeText } from "@/shared/lib/fs.js";
import { generateTaskId } from "../domain/task-id.js";
import {
  assertNoBlockCycle,
  assertNoParentCycle,
  validateTask,
} from "../domain/task-validators.js";
import {
  taskAlreadyClaimed,
  taskAlreadyCompleted,
  taskBlockedByOpenTasks,
  taskClaimBusySession,
  taskClaimOwnedByDifferentSession,
  taskNotClaimed,
  taskNotFound,
  taskReopenRequiresCompletedStatus,
  unknownBlocker,
} from "../domain/task-errors.js";
import { MaestroError } from "@/shared/errors.js";
import {
  DEFAULT_TASK_TYPE,
  DEFAULT_TASK_PRIORITY,
  DEFAULT_TASK_STATUS,
} from "../domain/task-types.js";
import {
  assertTaskMutationOwnership,
  assertTaskUpdateAllowed,
  findBusySessionTasks,
  getUnresolvedBlockerIds,
  releaseTaskOwnership,
} from "../domain/task-state.js";

const MAX_ID_RETRIES = 5;
const LOCK_WAIT_TIMEOUT_MS = 5_000;
const LOCK_INITIAL_RETRY_DELAY_MS = 10;
const LOCK_MAX_RETRY_DELAY_MS = 100;
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

  private batchesDir(): string {
    return join(this.tasksDir(), "batches");
  }

  private batchReceiptPath(batchId: string): string {
    assertBatchIdShape(batchId);
    return join(this.batchesDir(), `${batchId}.json`);
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

      if (input.parentId !== undefined && !tasks.has(input.parentId)) {
        throw taskNotFound(input.parentId);
      }
      ensureTasksExist("<new task>", input.blockedBy ?? [], tasks);

      const id = generateUniqueIds(1, tasks)[0]!;

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
        blocks: [],
        blockedBy: dedupeValues(input.blockedBy ?? []),
        createdAt: now,
        updatedAt: now,
      };

      tasks.set(id, task);
      for (const blockerId of task.blockedBy) {
        const blocker = tasks.get(blockerId)!;
        tasks.set(blockerId, {
          ...blocker,
          blocks: dedupeValues([...blocker.blocks, id]),
          updatedAt: now,
        });
      }

      await this.writeAll(tasks);
      return tasks.get(id)!;
    });
  }

  async createBatch(
    inputs: readonly CreateBatchInput[],
    receipt?: { readonly batchId: string; readonly names: readonly (string | undefined)[] },
  ): Promise<readonly Task[]> {
    if (inputs.length === 0) return [];

    return this.withLock(async () => {
      const tasks = await this.readAll();

      const generatedIds = generateUniqueIds(inputs.length, tasks);
      const resolveRef = (ref: number | string, source: "parent" | "blockedBy"): string => {
        if (typeof ref === "number") {
          if (ref < 0 || ref >= inputs.length) {
            throw new MaestroError(
              `Batch ${source} index ${ref} out of bounds (0..${inputs.length - 1})`,
              ["This indicates a bug in the caller that built the batch input"],
            );
          }
          return generatedIds[ref]!;
        }
        return ref;
      };

      for (const [idx, input] of inputs.entries()) {
        if (input.parentRef !== undefined && typeof input.parentRef === "string") {
          if (!tasks.has(input.parentRef)) {
            throw taskNotFound(input.parentRef);
          }
        }
        for (const blockerRef of input.blockedByRefs ?? []) {
          if (typeof blockerRef === "string" && !tasks.has(blockerRef)) {
            throw unknownBlocker(generatedIds[idx]!, [blockerRef]);
          }
        }
      }

      const now = new Date().toISOString();
      const proposed: Task[] = inputs.map((input, idx) => ({
        id: generatedIds[idx]!,
        title: input.title,
        description: input.description,
        type: input.type ?? DEFAULT_TASK_TYPE,
        priority: input.priority ?? DEFAULT_TASK_PRIORITY,
        status: DEFAULT_TASK_STATUS,
        parentId: input.parentRef === undefined ? undefined : resolveRef(input.parentRef, "parent"),
        labels: input.labels ?? [],
        blocks: [],
        blockedBy: dedupeValues((input.blockedByRefs ?? []).map((r) => resolveRef(r, "blockedBy"))),
        createdAt: now,
        updatedAt: now,
      }));

      for (const task of proposed) {
        tasks.set(task.id, task);
      }
      for (const task of proposed) {
        for (const blockerId of task.blockedBy) {
          const blocker = tasks.get(blockerId)!;
          tasks.set(blockerId, {
            ...blocker,
            blocks: dedupeValues([...blocker.blocks, task.id]),
            updatedAt: now,
          });
        }
      }

      for (const task of proposed) {
        if (task.parentId !== undefined) {
          assertNoParentCycle(task.id, task.parentId, tasks);
        }
        for (const blockerId of task.blockedBy) {
          assertNoBlockCycle(blockerId, [task.id], tasks);
        }
      }

      if (receipt !== undefined) {
        await this.writeReceiptFile({
          batchId: receipt.batchId,
          created: proposed.map((task, idx) => ({
            name: receipt.names[idx],
            id: task.id,
            status: task.status,
            assignee: task.assignee,
          })),
        });
      }

      await this.writeAll(tasks);
      return proposed.map((task) => tasks.get(task.id)!);
    });
  }

  async update(id: string, patch: UpdateTaskInput, opts: TaskMutationInput = {}): Promise<UpdateTaskResult> {
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
        assertNoParentCycle(id, patch.parentId, tasks);
      }

      const { nextStatus, autoClaim } = assertTaskUpdateAllowed(existing, patch, tasks, opts);

      const labels = applyLabelPatch(existing.labels, patch.addLabels, patch.removeLabels);
      const reason = patch.reason === undefined
        ? existing.closeReason
        : (patch.reason.length === 0 ? undefined : patch.reason);
      const now = new Date().toISOString();
      const receipt = buildReceiptForUpdate(existing, patch, nextStatus, reason, now);

      const updated: Task = {
        ...existing,
        title: patch.title ?? existing.title,
        description: patch.description !== undefined ? patch.description : existing.description,
        type: patch.type ?? existing.type,
        priority: patch.priority ?? existing.priority,
        status: nextStatus,
        parentId: patch.parentId === "" ? undefined : (patch.parentId ?? existing.parentId),
        labels,
        assignee: autoClaim ? autoClaim.sessionId : existing.assignee,
        claimedAt: autoClaim ? now : existing.claimedAt,
        closeReason: nextStatus === "completed" ? reason : existing.closeReason,
        receipt,
        updatedAt: now,
      };

      tasks.set(id, updated);
      await this.writeAll(tasks);
      return { task: updated, autoClaimed: autoClaim !== undefined };
    });
  }

  async claim(
    id: string,
    sessionId: string,
    opts: { force?: boolean; checkBusy?: boolean } = {},
  ): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const existing = tasks.get(id);
      if (!existing) {
        throw taskNotFound(id);
      }
      if (existing.status === "completed") {
        throw taskAlreadyCompleted(id);
      }
      if (existing.assignee && existing.assignee !== sessionId && !opts.force) {
        throw taskAlreadyClaimed(id, existing.assignee);
      }

      const blockers = getUnresolvedBlockerIds(existing, tasks);
      if (blockers.length > 0) {
        throw taskBlockedByOpenTasks(id, blockers);
      }

      if (opts.checkBusy) {
        const busy = findBusySessionTasks(sessionId, id, tasks);
        if (busy.length > 0) {
          throw taskClaimBusySession(sessionId, busy);
        }
      }

      if (existing.assignee === sessionId && existing.claimedAt !== undefined) {
        return existing;
      }

      const now = new Date().toISOString();
      const claimed: Task = {
        ...existing,
        assignee: sessionId,
        claimedAt: existing.claimedAt ?? now,
        updatedAt: now,
      };

      tasks.set(id, claimed);
      await this.writeAll(tasks);
      return claimed;
    });
  }

  async unclaim(id: string, sessionId: string, opts: { force?: boolean } = {}): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const existing = tasks.get(id);
      if (!existing) {
        throw taskNotFound(id);
      }
      if (existing.status === "completed") {
        throw taskAlreadyCompleted(id);
      }
      if (!existing.assignee) {
        throw taskNotClaimed(id);
      }
      if (existing.assignee !== sessionId && !opts.force) {
        throw taskClaimOwnedByDifferentSession(id, existing.assignee);
      }

      const now = new Date().toISOString();
      const unclaimed = releaseTaskOwnership(existing, now);

      tasks.set(id, unclaimed);
      await this.writeAll(tasks);
      return unclaimed;
    });
  }

  async block(
    id: string,
    blockedTaskIds: readonly string[],
    opts: TaskMutationInput = {},
  ): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const blocker = tasks.get(id);
      if (!blocker) {
        throw taskNotFound(id);
      }
      if (blocker.status === "completed") {
        throw taskAlreadyCompleted(id);
      }
      assertTaskMutationOwnership(blocker, opts, "block");

      ensureTasksExist(id, blockedTaskIds, tasks);
      assertNoBlockCycle(id, blockedTaskIds, tasks);

      const now = new Date().toISOString();
      let blockerChanged = false;
      blockerChanged = upsertBlockList(tasks, id, blocker, [...blocker.blocks, ...blockedTaskIds], now) || blockerChanged;

      for (const blockedTaskId of blockedTaskIds) {
        const blockedTask = tasks.get(blockedTaskId)!;
        if (blockedTask.status === "completed") {
          throw taskAlreadyCompleted(blockedTaskId);
        }
        assertTaskMutationOwnership(blockedTask, opts, "block");
        blockerChanged =
          upsertBlockedByList(tasks, blockedTaskId, blockedTask, [...blockedTask.blockedBy, id], now) ||
          blockerChanged;
      }

      if (!blockerChanged) {
        return tasks.get(id)!;
      }

      await this.writeAll(tasks);
      return tasks.get(id)!;
    });
  }

  async unblock(
    id: string,
    blockedTaskIds: readonly string[],
    opts: TaskMutationInput = {},
  ): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const blocker = tasks.get(id);
      if (!blocker) {
        throw taskNotFound(id);
      }
      if (blocker.status === "completed") {
        throw taskAlreadyCompleted(id);
      }
      assertTaskMutationOwnership(blocker, opts, "unblock");

      const removeSet = new Set(blockedTaskIds);
      const nextBlocks = blocker.blocks.filter((blockedId) => !removeSet.has(blockedId));
      const now = new Date().toISOString();
      let changed = upsertBlockList(tasks, id, blocker, nextBlocks, now);

      for (const blockedTaskId of blockedTaskIds) {
        const blockedTask = tasks.get(blockedTaskId);
        if (!blockedTask) {
          continue;
        }
        assertTaskMutationOwnership(blockedTask, opts, "unblock");
        const nextBlockedBy = blockedTask.blockedBy.filter((blockerId) => blockerId !== id);
        changed = upsertBlockedByList(tasks, blockedTaskId, blockedTask, nextBlockedBy, now) || changed;
      }

      if (!changed) {
        return tasks.get(id)!;
      }

      await this.writeAll(tasks);
      return tasks.get(id)!;
    });
  }

  async findBatchReceipt(batchId: string): Promise<BatchResult | undefined> {
    const path = this.batchReceiptPath(batchId);
    const raw = await readText(path);
    if (raw === undefined) return undefined;
    try {
      const parsed = JSON.parse(raw) as BatchResult;
      if (typeof parsed !== "object" || parsed === null || !Array.isArray(parsed.created)) {
        return undefined;
      }
      return parsed;
    } catch {
      return undefined;
    }
  }

  private async writeReceiptFile(result: BatchResult): Promise<void> {
    if (!result.batchId) return;
    const path = this.batchReceiptPath(result.batchId);
    await ensureDir(this.batchesDir());
    await writeText(path, `${JSON.stringify(result, null, 2)}\n`);
  }

  async releaseOwned(sessionId: string): Promise<readonly Task[]> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const now = new Date().toISOString();
      const released: Task[] = [];

      for (const [id, task] of tasks.entries()) {
        if (task.status === "completed" || task.assignee !== sessionId) {
          continue;
        }
        const updated = releaseTaskOwnership(task, now);
        tasks.set(id, updated);
        released.push(updated);
      }

      if (released.length === 0) {
        return [];
      }

      await this.writeAll(tasks);
      return released;
    });
  }

  async reopen(id: string): Promise<Task> {
    return this.withLock(async () => {
      const tasks = await this.readAll();
      const existing = tasks.get(id);
      if (!existing) {
        throw taskNotFound(id);
      }
      if (existing.status !== "completed") {
        throw taskReopenRequiresCompletedStatus(id);
      }

      const now = new Date().toISOString();
      const reopened: Task = {
        ...existing,
        status: "pending",
        assignee: undefined,
        claimedAt: undefined,
        closeReason: undefined,
        receipt: undefined,
        updatedAt: now,
      };

      tasks.set(id, reopened);
      await this.writeAll(tasks);
      return reopened;
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
    return normalizeGraph(result);
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
    const deadline = Date.now() + LOCK_WAIT_TIMEOUT_MS;
    let retryDelayMs = LOCK_INITIAL_RETRY_DELAY_MS;

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
        if (Date.now() >= deadline) {
          throw new MaestroError(`Task store lock is still active: ${lockPath}`, [
            "Retry once the other task command finishes",
            `If this lock is stale, remove it manually: rm ${lockPath}`,
          ]);
        }
        await sleep(retryDelayMs);
        retryDelayMs = Math.min(retryDelayMs * 2, LOCK_MAX_RETRY_DELAY_MS);
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

function buildReceiptForUpdate(
  existing: Task,
  patch: UpdateTaskInput,
  nextStatus: Task["status"],
  reason: string | undefined,
  now: string,
): TaskReceipt | undefined {
  if (nextStatus !== "completed") {
    return existing.receipt;
  }

  const summaryFromPatch = patch.summary?.trim();
  const summary = summaryFromPatch && summaryFromPatch.length > 0
    ? summaryFromPatch
    : reason;

  const surprise = patch.surprise?.trim();
  const verifiedBy = patch.verifiedBy?.filter((name) => name.length > 0) ?? [];

  const hasAnyField = (summary !== undefined && summary.length > 0)
    || (surprise !== undefined && surprise.length > 0)
    || verifiedBy.length > 0;

  if (!hasAnyField) {
    return existing.receipt;
  }

  return {
    summary: summary ?? "",
    ...(surprise && surprise.length > 0 ? { surprise } : {}),
    ...(verifiedBy.length > 0 ? { verifiedBy } : {}),
    capturedAt: now,
  };
}

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

function ensureTasksExist(
  id: string,
  taskIds: readonly string[],
  tasks: ReadonlyMap<string, Task>,
): void {
  const missing = taskIds.filter((taskId) => !tasks.has(taskId));
  if (missing.length > 0) {
    throw unknownBlocker(id, missing);
  }
}

const BATCH_ID_PATTERN = /^[a-zA-Z0-9._-]{1,64}$/;

function assertBatchIdShape(batchId: string): void {
  if (!BATCH_ID_PATTERN.test(batchId)) {
    throw new MaestroError(`Invalid batchId '${batchId}'`, [
      "batchId must match /^[a-zA-Z0-9._-]{1,64}$/",
      "Pick a stable identifier the agent can re-submit verbatim for idempotent replay",
    ]);
  }
}

function generateUniqueIds(
  count: number,
  existing: ReadonlyMap<string, Task>,
): readonly string[] {
  const ids: string[] = [];
  const claimed = new Set<string>();
  for (let i = 0; i < count; i++) {
    let id: string | undefined;
    for (let attempt = 0; attempt < MAX_ID_RETRIES; attempt++) {
      const candidate = generateTaskId();
      if (!existing.has(candidate) && !claimed.has(candidate)) {
        id = candidate;
        break;
      }
    }
    if (id === undefined) {
      throw new MaestroError(
        `Failed to generate a unique task id after ${MAX_ID_RETRIES} attempts`,
        [
          "Retry the batch to generate fresh task ids",
          "If the problem persists, inspect .maestro/tasks/tasks.jsonl for id collisions",
        ],
      );
    }
    ids.push(id);
    claimed.add(id);
  }
  return ids;
}

function normalizeGraph(tasks: Map<string, Task>): Map<string, Task> {
  let normalized: Map<string, Task> | undefined;

  const ensureMutable = (): Map<string, Task> => {
    if (!normalized) {
      normalized = new Map(tasks);
    }
    return normalized;
  };

  for (const [id, task] of tasks.entries()) {
    const nextBlocks = dedupeValues(task.blocks);
    const nextBlockedBy = dedupeValues(task.blockedBy);
    if (!sameValues(task.blocks, nextBlocks) || !sameValues(task.blockedBy, nextBlockedBy)) {
      ensureMutable().set(id, {
        ...task,
        blocks: nextBlocks,
        blockedBy: nextBlockedBy,
      });
    }
  }

  const source = normalized ?? tasks;
  for (const task of source.values()) {
    for (const blockedId of task.blocks) {
      const blockedTask = (normalized ?? tasks).get(blockedId);
      if (!blockedTask || blockedTask.blockedBy.includes(task.id)) continue;
      upsertBlockedByList(
        ensureMutable(),
        blockedId,
        blockedTask,
        [...blockedTask.blockedBy, task.id],
        blockedTask.updatedAt,
      );
    }
    for (const blockerId of task.blockedBy) {
      const blockerTask = (normalized ?? tasks).get(blockerId);
      if (!blockerTask || blockerTask.blocks.includes(task.id)) continue;
      upsertBlockList(
        ensureMutable(),
        blockerId,
        blockerTask,
        [...blockerTask.blocks, task.id],
        blockerTask.updatedAt,
      );
    }
  }

  return normalized ?? tasks;
}

function upsertBlockList(
  tasks: Map<string, Task>,
  id: string,
  task: Task,
  nextBlocksRaw: readonly string[],
  updatedAt: string,
): boolean {
  const nextBlocks = dedupeValues(nextBlocksRaw);
  if (sameValues(task.blocks, nextBlocks)) {
    return false;
  }
  tasks.set(id, {
    ...task,
    blocks: nextBlocks,
    updatedAt,
  });
  return true;
}

function upsertBlockedByList(
  tasks: Map<string, Task>,
  id: string,
  task: Task,
  nextBlockedByRaw: readonly string[],
  updatedAt: string,
): boolean {
  const nextBlockedBy = dedupeValues(nextBlockedByRaw);
  if (sameValues(task.blockedBy, nextBlockedBy)) {
    return false;
  }
  tasks.set(id, {
    ...task,
    blockedBy: nextBlockedBy,
    updatedAt,
  });
  return true;
}

function dedupeValues(values: readonly string[]): readonly string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    if (!seen.has(value)) {
      result.push(value);
      seen.add(value);
    }
  }
  return result;
}

function sameValues(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
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
