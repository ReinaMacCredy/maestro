import type { TaskQueryPort } from "@/features/task";
import type { HandoffStorePort } from "../domain/handoff-types.js";
import { listOpenProjectHandoffIdsForTask } from "./read-handoffs.usecase.js";

// Returns plain ids (not records) so the task feature can depend on this
// without importing any handoff domain types.
export async function listOpenHandoffsForTask(
  store: HandoffStorePort,
  taskId: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
    readonly currentProjectRoot?: string;
  } = {},
): Promise<readonly string[]> {
  return listOpenProjectHandoffIdsForTask(store, taskId, options);
}
