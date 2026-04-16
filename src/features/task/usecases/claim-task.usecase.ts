import type { Task, ClaimTaskInput } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function claimTask(
  store: TaskStorePort,
  id: string,
  input: ClaimTaskInput,
): Promise<Task> {
  return store.claim(id, input.sessionId, {
    force: input.force,
    checkBusy: input.checkBusy,
  });
}
