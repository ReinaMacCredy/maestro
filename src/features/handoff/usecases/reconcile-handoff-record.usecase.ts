import type { TaskQueryPort } from "@/shared/domain/legacy-task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { isOpenHandoffRecord } from "../domain/handoff-state.js";
import { isHandoffInProject } from "../domain/project-scope.js";

export async function reconcileHandoffRecord(
  deps: {
    readonly handoffStore: HandoffStorePort;
    readonly taskStore: Pick<TaskQueryPort, "get">;
    readonly currentProjectRoot?: string;
  },
  record: HandoffRecord,
): Promise<HandoffRecord> {
  if (
    !record.refs.taskId
    || !deps.currentProjectRoot
    || !isOpenHandoffRecord(record)
    || !isHandoffInProject(record, deps.currentProjectRoot)
  ) {
    return record;
  }

  const linkedTask = await deps.taskStore.get(record.refs.taskId);
  if (linkedTask?.status !== "completed") {
    return record;
  }

  const reconciled: HandoffRecord = {
    ...record,
    status: "completed",
  };
  return deps.handoffStore.update(reconciled);
}
