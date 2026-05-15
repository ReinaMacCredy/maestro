import type { Task } from "../domain/task-types.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export async function heartbeatTask(
  store: TaskStorePort,
  id: string,
  sessionId: string,
  opts: { force?: boolean } = {},
): Promise<Task> {
  return store.heartbeat(id, sessionId, opts);
}
