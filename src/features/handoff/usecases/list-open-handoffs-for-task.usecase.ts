import type { TaskQueryPort } from "@/features/task";
import type { LaunchStorePort } from "../domain/launch-types.js";
import { isOpenLaunchRecord } from "../domain/launch-state.js";
import { reconcileLaunchRecord } from "./reconcile-launch-record.usecase.js";

// Returns plain ids (not records) so the task feature can depend on this
// without importing any handoff domain types.
export async function listOpenHandoffsForTask(
  store: LaunchStorePort,
  taskId: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
  } = {},
): Promise<readonly string[]> {
  const all = await store.list();
  const relevantOpen = all.filter((record) => record.refs.taskId === taskId && isOpenLaunchRecord(record));
  const reconciled = options.taskStore
    ? await Promise.all(relevantOpen.map((record) => reconcileLaunchRecord({
      launchStore: store,
      taskStore: options.taskStore!,
    }, record)))
    : relevantOpen;
  return reconciled
    .filter(isOpenLaunchRecord)
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .map((record) => record.id);
}
