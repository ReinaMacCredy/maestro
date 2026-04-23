import type { TaskQueryPort } from "@/features/task";
import type { HandoffStorePort } from "../domain/handoff-types.js";
import { isOpenHandoffRecord } from "../domain/handoff-state.js";
import { reconcileHandoffRecord } from "./reconcile-handoff-record.usecase.js";

// Returns plain ids (not records) so the task feature can depend on this
// without importing any handoff domain types.
export async function listOpenHandoffsForTask(
  store: HandoffStorePort,
  taskId: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
  } = {},
): Promise<readonly string[]> {
  const all = await store.list();
  const relevantOpen = all.filter((record) => record.refs.taskId === taskId && isOpenHandoffRecord(record));
  const reconciled = options.taskStore
    ? await Promise.all(relevantOpen.map((record) => reconcileHandoffRecord({
      handoffStore: store,
      taskStore: options.taskStore!,
    }, record)))
    : relevantOpen;
  return reconciled
    .filter(isOpenHandoffRecord)
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .map((record) => record.id);
}
