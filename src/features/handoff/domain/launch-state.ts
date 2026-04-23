import type { HandoffLaunchRecord } from "./launch-types.js";

export type HandoffLaunchDisplayState = "open" | "consumed" | "completed" | "failed";

export function isOpenLaunchRecord(record: HandoffLaunchRecord): boolean {
  return !record.consumedAt && (record.status === "launching" || record.status === "launched");
}

export function getLaunchDisplayState(record: HandoffLaunchRecord): HandoffLaunchDisplayState {
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
