import type { TaskQueryPort } from "@/features/task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { MaestroError } from "@/shared/errors.js";
import { reconcileHandoffRecord } from "./reconcile-handoff-record.usecase.js";

export async function showHandoff(
  store: HandoffStorePort,
  id: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
  } = {},
): Promise<HandoffRecord> {
  const record = await store.get(id);
  if (!record) {
    throw new MaestroError(`Handoff packet not found: ${id}`, [
      "Run `maestro handoff list` to see available packets",
    ]);
  }
  if (!options.taskStore) {
    return record;
  }
  return reconcileHandoffRecord({ handoffStore: store, taskStore: options.taskStore }, record);
}
