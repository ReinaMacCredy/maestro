import type { TaskQueryPort } from "@/features/task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { isOpenHandoffRecord } from "../domain/handoff-state.js";

export async function reconcileHandoffRecord(
  deps: {
    readonly handoffStore: HandoffStorePort;
    readonly taskStore: Pick<TaskQueryPort, "get">;
  },
  record: HandoffRecord,
): Promise<HandoffRecord> {
  if (!record.refs.taskId || !isOpenHandoffRecord(record)) {
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
