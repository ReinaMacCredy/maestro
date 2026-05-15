import type { TaskQueryPort } from "@/shared/domain/legacy-task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { listAllHandoffs } from "./read-handoffs.usecase.js";

export interface ListHandoffsOptions {
  readonly openOnly?: boolean;
  readonly taskStore?: Pick<TaskQueryPort, "get">;
  readonly currentProjectRoot?: string;
}

export async function listHandoffs(
  store: HandoffStorePort,
  options: ListHandoffsOptions = {},
): Promise<readonly HandoffRecord[]> {
  return listAllHandoffs(store, options);
}
