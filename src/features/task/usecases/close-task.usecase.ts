import type { Task, CloseTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function closeTask(
  store: TaskStorePort,
  id: string,
  input: CloseTaskInput,
): Promise<Task> {
  return store.close(id, input);
}
