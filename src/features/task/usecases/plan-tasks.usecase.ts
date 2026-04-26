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
  deriveSlugFromTitle,
  isValidSlugShape,
} from "../domain/task-slug.js";
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
const MAX_DERIVED_SLUG_SUFFIX = 9;

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
    if (replay) return { ...replay, replayed: true };
  }

  const nameToIndex = buildNameIndex(input.tasks);
  const validationIssues = collectValidationIssues(input.tasks);
  if (validationIssues.length > 0) {
    throw batchValidationErrors(validationIssues);
  }

  const existingTopLevelSlugs = await collectExistingTopLevelSlugs(store);
  const resolvedSlugs = resolveSlugs(input.tasks, existingTopLevelSlugs);
  const slugToIndex = buildSlugIndex(resolvedSlugs);

  const createInputs: CreateBatchInput[] = input.tasks.map((task, idx) =>
    buildCreateBatchInput(task, idx, nameToIndex, slugToIndex, resolvedSlugs),
  );

  const receiptMeta = input.batchId === undefined
    ? undefined
    : { batchId: input.batchId, names: input.tasks.map((t) => t.name) };

  const created = await store.createBatch(createInputs, receiptMeta);

  const results: BatchCreatedTask[] = created.map((task, idx) => ({
    name: input.tasks[idx]!.name,
    id: task.id,
    status: task.status,
    assignee: task.assignee,
  }));

  return {
    batchId: input.batchId,
    created: results,
  };
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
    if (task.slug !== undefined) {
      if (typeof task.slug !== "string") {
        issues.push(`${label}: 'slug' must be a string`);
      } else if (task.parent !== undefined) {
        issues.push(
          `${label}: 'slug' is forbidden on step entries (drop 'slug' or drop 'parent')`,
        );
      } else if (!isValidSlugShape(task.slug)) {
        issues.push(
          `${label}: 'slug' '${task.slug}' must be '<verb>/<kebab>' (verbs: implement, fix, chore, spike, epic; kebab is lowercase ASCII; total <= 60 chars)`,
        );
      }
    }
  }
  return issues;
}

function taskLabel(idx: number, task: BatchTaskInput): string {
  const name = task.name;
  const nameSuffix = name ? ` (name '${name}')` : "";
  return `Task #${idx + 1}${nameSuffix}`;
}

async function collectExistingTopLevelSlugs(store: TaskStorePort): Promise<Set<string>> {
  const slugs = new Set<string>();
  const all = await store.all();
  for (const task of all) {
    if (task.parentId === undefined && task.slug !== undefined) {
      slugs.add(task.slug);
    }
  }
  return slugs;
}

/**
 * Walk every batch entry and assign a slug to top-level entries (PC1, PC3,
 * PC4, PC5, PC7, PC9). Returns a parallel array of slugs keyed by index;
 * entries with `parent` set get `undefined`. Throws atomically before any
 * write happens.
 */
function resolveSlugs(
  tasks: readonly BatchTaskInput[],
  existingTopLevelSlugs: ReadonlySet<string>,
): readonly (string | undefined)[] {
  const issues: string[] = [];
  const usedInBatch = new Set<string>();
  const result: (string | undefined)[] = new Array(tasks.length).fill(undefined);

  for (const [idx, task] of tasks.entries()) {
    if (task.parent !== undefined) continue;

    if (task.slug !== undefined && task.slug.length > 0) {
      const slug = task.slug;
      if (existingTopLevelSlugs.has(slug)) {
        issues.push(
          `${taskLabel(idx, task)}: slug '${slug}' is already used by an existing top-level task`,
        );
        continue;
      }
      if (usedInBatch.has(slug)) {
        issues.push(
          `${taskLabel(idx, task)}: slug '${slug}' collides with another entry in the same batch`,
        );
        continue;
      }
      usedInBatch.add(slug);
      result[idx] = slug;
      continue;
    }

    let derived: string;
    try {
      derived = deriveSlugFromTitle(task.title.trim(), task.type);
    } catch (error) {
      issues.push(
        `${taskLabel(idx, task)}: cannot derive a slug from title '${task.title}'; pass 'slug' explicitly`,
      );
      continue;
    }

    const candidate = pickFreeDerivedSlug(derived, existingTopLevelSlugs, usedInBatch);
    if (candidate === undefined) {
      issues.push(
        `${taskLabel(idx, task)}: derived slug '${derived}' and suffixes -2..-${MAX_DERIVED_SLUG_SUFFIX} are all in use; pass 'slug' explicitly`,
      );
      continue;
    }

    usedInBatch.add(candidate);
    result[idx] = candidate;
  }

  if (issues.length > 0) {
    throw batchValidationErrors(issues);
  }

  return result;
}

function pickFreeDerivedSlug(
  base: string,
  existing: ReadonlySet<string>,
  usedInBatch: ReadonlySet<string>,
): string | undefined {
  const isFree = (slug: string): boolean => !existing.has(slug) && !usedInBatch.has(slug);
  if (isFree(base)) return base;
  for (let suffix = 2; suffix <= MAX_DERIVED_SLUG_SUFFIX; suffix++) {
    const candidate = `${base}-${suffix}`;
    if (isFree(candidate)) return candidate;
  }
  return undefined;
}

function buildSlugIndex(
  resolvedSlugs: readonly (string | undefined)[],
): ReadonlyMap<string, number> {
  const map = new Map<string, number>();
  for (const [idx, slug] of resolvedSlugs.entries()) {
    if (slug !== undefined) {
      map.set(slug, idx);
    }
  }
  return map;
}

function buildCreateBatchInput(
  task: BatchTaskInput,
  idx: number,
  nameToIndex: ReadonlyMap<string, number>,
  slugToIndex: ReadonlyMap<string, number>,
  resolvedSlugs: readonly (string | undefined)[],
): CreateBatchInput {
  return {
    title: task.title.trim(),
    description: task.description,
    type: task.type,
    priority: task.priority,
    labels: task.labels,
    parentRef: task.parent === undefined
      ? undefined
      : resolveReference(task.parent, "parent", nameToIndex, slugToIndex),
    slug: resolvedSlugs[idx],
    blockedByRefs: (task.blockedBy ?? []).map((ref) =>
      resolveReference(ref, "blockedBy", nameToIndex, slugToIndex),
    ),
  };
}

function resolveReference(
  raw: string,
  source: "parent" | "blockedBy",
  nameToIndex: ReadonlyMap<string, number>,
  slugToIndex: ReadonlyMap<string, number>,
): number | string {
  if (TASK_ID_PATTERN.test(raw)) {
    return raw;
  }
  const nameIdx = nameToIndex.get(raw);
  if (nameIdx !== undefined) {
    return nameIdx;
  }
  const slugIdx = slugToIndex.get(raw);
  if (slugIdx !== undefined) {
    return slugIdx;
  }
  throw batchUnknownReference(raw, source);
}
