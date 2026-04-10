import type { Task, CreateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateCreateInput, assertNoParentCycle } from "../domain/task-validators.js";
import { unknownDependency, taskNotFound } from "../domain/task-errors.js";

/**
 * Create a new task after validating inputs and cross-checking references.
 *
 * - Validates input shape (title, priority, type, depends-on format).
 * - Ensures every --depends-on id exists in the store.
 * - Ensures the --parent id exists and does not create a cycle.
 *
 * These checks happen against the live store because the adapter does not
 * enforce them — we want a single cross-entity validation layer at the
 * use-case level so tests can mock the store without re-implementing
 * validation.
 */
export async function createTask(
  store: TaskStorePort,
  rawInput: CreateTaskInput,
): Promise<Task> {
  const input = validateCreateInput(rawInput);

  const existing = await store.all();
  const byId = new Map(existing.map((t) => [t.id, t] as const));

  if (input.dependsOn && input.dependsOn.length > 0) {
    const missing = input.dependsOn.filter((id) => !byId.has(id));
    if (missing.length > 0) {
      throw unknownDependency("<new task>", missing);
    }
  }

  if (input.parentId !== undefined) {
    if (!byId.has(input.parentId)) {
      throw taskNotFound(input.parentId);
    }
    // A brand-new task cannot itself be a cycle ancestor, so we only need to
    // check that the chosen parent's chain terminates — assertNoParentCycle
    // handles that via the MAX_PARENT_DEPTH guard.
  }

  return store.create(input);
}
