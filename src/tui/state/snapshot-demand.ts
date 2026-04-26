import type { SnapshotBuildOptions } from "./snapshot.js";

export type MissionControlSnapshotDemandMode =
  | "json"
  | "render-check"
  | "preview-all"
  | "preview-screen"
  | "interactive";

export interface MissionControlSnapshotDemandInput {
  readonly mode: MissionControlSnapshotDemandMode;
  readonly screen?: string;
}

export function buildMissionControlSnapshotDemand(
  input: MissionControlSnapshotDemandInput,
): SnapshotBuildOptions {
  switch (input.mode) {
    case "json":
    case "render-check":
    case "preview-all":
      return { includeTaskBoard: true };
    case "preview-screen":
      return { includeTaskBoard: input.screen === "tasks" };
    case "interactive":
      return {};
  }
}
