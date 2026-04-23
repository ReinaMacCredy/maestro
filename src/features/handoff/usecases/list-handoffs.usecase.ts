import type { TaskQueryPort } from "@/features/task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { isOpenHandoffRecord } from "../domain/handoff-state.js";
import { reconcileHandoffRecord } from "./reconcile-handoff-record.usecase.js";

export interface ListHandoffsOptions {
  readonly openOnly?: boolean;
  readonly taskStore?: Pick<TaskQueryPort, "get">;
}

export async function listHandoffs(
  store: HandoffStorePort,
  options: ListHandoffsOptions = {},
): Promise<readonly HandoffRecord[]> {
  const { taskStore } = options;
  const all = await store.list();
  const reconciled = taskStore
    ? await Promise.all(all.map((record) => reconcileHandoffRecord({
      handoffStore: store,
      taskStore,
    }, record)))
    : all;
  const filtered = options.openOnly ? reconciled.filter(isOpenHandoffRecord) : reconciled;
  return [...filtered].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
}
