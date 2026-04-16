import type { Task } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function releaseOwnedTasks(
  store: TaskStorePort,
  sessionId: string,
): Promise<readonly Task[]> {
  return store.releaseOwned(sessionId);
}
