import type { TaskQueryPort } from "@/features/task";
import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";
import { MaestroError } from "@/shared/errors.js";
import { reconcileLaunchRecord } from "./reconcile-launch-record.usecase.js";

export async function showLaunch(
  store: LaunchStorePort,
  id: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
  } = {},
): Promise<HandoffLaunchRecord> {
  const record = await store.get(id);
  if (!record) {
    throw new MaestroError(`Handoff packet not found: ${id}`, [
      "Run `maestro handoff list` to see available packets",
    ]);
  }
  if (!options.taskStore) {
    return record;
  }
  return reconcileLaunchRecord({ launchStore: store, taskStore: options.taskStore }, record);
}
