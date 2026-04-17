import type {
  BatchCreatedTask,
  BatchInput,
  BatchResult,
  BatchTaskInput,
  CreateBatchInput,
} from "../domain/task-batch-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { TASK_ID_PATTERN } from "../domain/task-id.js";
import { TASK_PRIORITIES, TASK_TYPES } from "../domain/task-types.js";
import { isTaskPriority, isTaskType } from "../domain/task-validators.js";
import {
  batchDuplicateName,
  batchMalformedInput,
  batchNameLooksLikeTaskId,
  batchSizeExceeded,
  batchStaleReceipt,
  batchUnknownReference,
  batchValidationErrors,
} from "../domain/task-errors.js";

const MAX_BATCH_SIZE = 500;

export interface PlanTasksOptions {
  readonly maxBatchSize?: number;
}

export async function planTasks(
  store: TaskStorePort,
  input: BatchInput,
  options: PlanTasksOptions = {},
): Promise<BatchResult> {
  const maxBatchSize = options.maxBatchSize ?? MAX_BATCH_SIZE;

  if (!Array.isArray(input.tasks) || input.tasks.length === 0) {
    throw batchMalformedInput("'tasks' must be a non-empty array");
  }
  if (input.tasks.length > maxBatchSize) {
    throw batchSizeExceeded(input.tasks.length, maxBatchSize);
  }

  if (input.batchId !== undefined) {
    const replay = await tryReplayReceipt(store, input.batchId);
    if (replay) return replay;
  }

  const nameToIndex = buildNameIndex(input.tasks);
  const validationIssues = collectValidationIssues(input.tasks);
  if (validationIssues.length > 0) {
    throw batchValidationErrors(validationIssues);
  }

  const createInputs: CreateBatchInput[] = input.tasks.map((task, idx) =>
    buildCreateBatchInput(task, idx, nameToIndex),
  );

  const created = await store.createBatch(createInputs);

  const results: BatchCreatedTask[] = created.map((task, idx) => ({
    name: input.tasks[idx]!.name,
    id: task.id,
    status: task.status,
    assignee: task.assignee,
  }));

  const result: BatchResult = {
    batchId: input.batchId,
    created: results,
  };

  if (input.batchId !== undefined) {
    await store.writeBatchReceipt(result);
  }

  return result;
}

async function tryReplayReceipt(
  store: TaskStorePort,
  batchId: string,
): Promise<BatchResult | undefined> {
  const receipt = await store.findBatchReceipt(batchId);
  if (!receipt) return undefined;

  const liveTasks = await store.all();
  const liveIds = new Set(liveTasks.map((task) => task.id));
  const missing = receipt.created.filter((t) => !liveIds.has(t.id)).map((t) => t.id);
  if (missing.length > 0) {
    throw batchStaleReceipt(batchId, missing);
  }
  return receipt;
}

function buildNameIndex(tasks: readonly BatchTaskInput[]): ReadonlyMap<string, number> {
  const nameToIndex = new Map<string, number>();
  for (const [idx, task] of tasks.entries()) {
    if (task.name === undefined) continue;
    if (TASK_ID_PATTERN.test(task.name)) {
      throw batchNameLooksLikeTaskId(task.name);
    }
    if (nameToIndex.has(task.name)) {
      throw batchDuplicateName(task.name);
    }
    nameToIndex.set(task.name, idx);
  }
  return nameToIndex;
}

function collectValidationIssues(tasks: readonly BatchTaskInput[]): readonly string[] {
  const issues: string[] = [];
  for (const [idx, task] of tasks.entries()) {
    const label = taskLabel(idx, task);
    if (typeof task.title !== "string" || task.title.trim().length === 0) {
      issues.push(`${label}: 'title' must be a non-empty string`);
    }
    if (task.type !== undefined && !isTaskType(task.type)) {
      issues.push(`${label}: 'type' must be one of ${TASK_TYPES.join(", ")}`);
    }
    if (task.priority !== undefined && !isTaskPriority(task.priority)) {
      issues.push(`${label}: 'priority' must be one of ${TASK_PRIORITIES.join(", ")}`);
    }
    if (task.labels !== undefined) {
      if (!Array.isArray(task.labels)) {
        issues.push(`${label}: 'labels' must be an array of strings`);
      } else {
        for (const value of task.labels) {
          if (typeof value !== "string" || value.length === 0) {
            issues.push(`${label}: labels must be non-empty strings`);
            break;
          }
        }
      }
    }
    if (task.blockedBy !== undefined && !Array.isArray(task.blockedBy)) {
      issues.push(`${label}: 'blockedBy' must be an array of strings`);
    }
    if (task.parent !== undefined && typeof task.parent !== "string") {
      issues.push(`${label}: 'parent' must be a string`);
    }
  }
  return issues;
}

function taskLabel(idx: number, task: BatchTaskInput): string {
  const name = task.name;
  const nameSuffix = name ? ` (name '${name}')` : "";
  return `Task #${idx + 1}${nameSuffix}`;
}

function buildCreateBatchInput(
  task: BatchTaskInput,
  idx: number,
  nameToIndex: ReadonlyMap<string, number>,
): CreateBatchInput {
  return {
    title: task.title.trim(),
    description: task.description,
    type: task.type,
    priority: task.priority,
    labels: task.labels,
    parentRef: task.parent === undefined
      ? undefined
      : resolveReference(task.parent, "parent", nameToIndex),
    blockedByRefs: (task.blockedBy ?? []).map((ref) =>
      resolveReference(ref, "blockedBy", nameToIndex),
    ),
  };
}

function resolveReference(
  raw: string,
  source: "parent" | "blockedBy",
  nameToIndex: ReadonlyMap<string, number>,
): number | string {
  if (TASK_ID_PATTERN.test(raw)) {
    return raw;
  }
  const idx = nameToIndex.get(raw);
  if (idx === undefined) {
    throw batchUnknownReference(raw, source);
  }
  return idx;
}
