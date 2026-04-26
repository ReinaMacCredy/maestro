import type { Task, CreateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateCreateInput } from "../domain/task-validators.js";
import {
  collectExistingTopLevelSlugs,
  deriveSlugFromTitle,
  isValidSlugShape,
  pickFreeDerivedSlug,
} from "../domain/task-slug.js";
import { slugCollision } from "../domain/task-errors.js";

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
  const existing = await collectExistingTopLevelSlugs(store);
  const candidate = pickFreeDerivedSlug(baseSlug, existing);
  if (candidate === undefined) {
    throw slugCollision(baseSlug, "(numeric suffixes -2..-9 exhausted)");
  }
  return store.create({ ...input, slug: candidate });
}
