import type { HandoffRecord } from "./handoff-types.js";

export type HandoffDisplayState = "open" | "consumed" | "completed" | "failed";

export function isOpenHandoffRecord(record: HandoffRecord): boolean {
  return !record.consumedAt && (record.status === "launching" || record.status === "launched");
}

export function getHandoffDisplayState(record: HandoffRecord): HandoffDisplayState {
  if (record.consumedAt || record.status === "consumed") {
    return "consumed";
  }
  if (record.status === "failed") {
    return "failed";
  }
  if (record.status === "completed") {
    return "completed";
  }
  return "open";
}
