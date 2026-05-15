import type { TaskQueryPort } from "@/shared/domain/legacy-task";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { showAnyHandoff } from "./read-handoffs.usecase.js";

export async function showHandoff(
  store: HandoffStorePort,
  id: string,
  options: {
    readonly taskStore?: Pick<TaskQueryPort, "get">;
    readonly currentProjectRoot?: string;
  } = {},
): Promise<HandoffRecord> {
  return showAnyHandoff(store, id, options);
}
