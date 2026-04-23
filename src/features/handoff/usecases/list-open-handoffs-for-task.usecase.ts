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
  const reconciled = options.taskStore
    ? await Promise.all(all.map((record) => reconcileLaunchRecord({
      launchStore: store,
      taskStore: options.taskStore!,
    }, record)))
    : all;
  return reconciled
    .filter((record) => isOpenLaunchRecord(record) && record.refs.taskId === taskId)
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .map((record) => record.id);
}
