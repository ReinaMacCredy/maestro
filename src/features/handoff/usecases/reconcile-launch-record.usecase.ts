import type { TaskQueryPort } from "@/features/task";
import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";
import { isOpenLaunchRecord } from "../domain/launch-state.js";

export async function reconcileLaunchRecord(
  deps: {
    readonly launchStore: LaunchStorePort;
    readonly taskStore: Pick<TaskQueryPort, "get">;
  },
  record: HandoffLaunchRecord,
): Promise<HandoffLaunchRecord> {
  if (!record.refs.taskId || !isOpenLaunchRecord(record)) {
    return record;
  }

  const linkedTask = await deps.taskStore.get(record.refs.taskId);
  if (linkedTask?.status !== "completed") {
    return record;
  }

  const reconciled: HandoffLaunchRecord = {
    ...record,
    status: "completed",
  };
  return deps.launchStore.update(reconciled);
}
