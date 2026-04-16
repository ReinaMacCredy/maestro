import type { Task, UnclaimTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function unclaimTask(
  store: TaskStorePort,
  id: string,
  input: UnclaimTaskInput,
): Promise<Task> {
  return store.unclaim(id, input.sessionId, { force: input.force });
}
