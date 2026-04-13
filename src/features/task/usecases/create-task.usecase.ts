import type { Task, CreateTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateCreateInput } from "../domain/task-validators.js";

/**
 * Create a new task after validating inputs and cross-checking references.
 *
 * Input shape validation stays here so callers get domain-level errors before
 * the persistence layer runs. Cross-task reference checks happen inside the
 * store's locked mutation path so create only parses storage once.
 */
export async function createTask(
  store: TaskStorePort,
  rawInput: CreateTaskInput,
): Promise<Task> {
  const input = validateCreateInput(rawInput);
  return store.create(input);
}
