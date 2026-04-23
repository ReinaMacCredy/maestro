import type { TaskQueryPort } from "@/features/task";
import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";
import { isOpenLaunchRecord } from "../domain/launch-state.js";
import { reconcileLaunchRecord } from "./reconcile-launch-record.usecase.js";

export interface ListLaunchesOptions {
  readonly openOnly?: boolean;
  readonly taskStore?: Pick<TaskQueryPort, "get">;
}

export async function listLaunches(
  store: LaunchStorePort,
  options: ListLaunchesOptions = {},
): Promise<readonly HandoffLaunchRecord[]> {
  const all = await store.list();
  const reconciled = options.taskStore
    ? await Promise.all(all.map((record) => reconcileLaunchRecord({
      launchStore: store,
      taskStore: options.taskStore!,
    }, record)))
    : all;
  const filtered = options.openOnly ? reconciled.filter(isOpenLaunchRecord) : reconciled;
  return [...filtered].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
}
