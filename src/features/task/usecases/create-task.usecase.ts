import type { Task, CreateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateCreateInput } from "../domain/task-validators.js";
import { deriveSlugFromTitle, isValidSlugShape } from "../domain/task-slug.js";
import { slugCollision } from "../domain/task-errors.js";

const MAX_SLUG_SUFFIX_ATTEMPTS = 9;

/**
 * Create a new task after validating inputs and cross-checking references.
 *
 * For top-level tasks (no parentId), a slug is mandatory: callers either pass
 * one explicitly or let `deriveSlugFromTitle` produce one. On collision the
 * derived slug is suffixed `-2..-9` before giving up; an explicit slug never
 * gets a suffix and surfaces `slugCollision` directly.
 */
export async function createTask(
  store: TaskStorePort,
  rawInput: CreateTaskInput,
): Promise<Task> {
  const input = validateCreateInput(rawInput);

  if (input.parentId !== undefined) {
    return store.create(input);
  }

  if (input.slug !== undefined && isValidSlugShape(input.slug)) {
    return store.create(input);
  }

  const baseSlug = deriveSlugFromTitle(input.title, input.type);
  const existingTopLevelSlugs = await collectTopLevelSlugs(store);

  const candidates = [baseSlug];
  for (let suffix = 2; suffix <= MAX_SLUG_SUFFIX_ATTEMPTS; suffix++) {
    candidates.push(`${baseSlug}-${suffix}`);
  }

  let lastCollision: string | undefined;
  for (const candidate of candidates) {
    if (existingTopLevelSlugs.has(candidate)) {
      lastCollision = candidate;
      continue;
    }
    return store.create({ ...input, slug: candidate });
  }

  throw slugCollision(
    lastCollision ?? baseSlug,
    `(${candidates.length} candidates exhausted)`,
  );
}

async function collectTopLevelSlugs(store: TaskStorePort): Promise<Set<string>> {
  const all = await store.all();
  const slugs = new Set<string>();
  for (const task of all) {
    if (task.parentId === undefined && task.slug !== undefined) {
      slugs.add(task.slug);
    }
  }
  return slugs;
}
