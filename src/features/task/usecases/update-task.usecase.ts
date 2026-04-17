import type {
  TaskMutationInput,
  UpdateTaskInput,
  UpdateTaskResult,
} from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";
import { validateUpdateInput } from "../domain/task-validators.js";

export async function updateTask(
  store: TaskStorePort,
  id: string,
  patch: UpdateTaskInput,
  opts: TaskMutationInput = {},
): Promise<UpdateTaskResult> {
  const validated = validateUpdateInput(patch);
  return store.update(id, validated, opts);
}
