// Build a MissionControlSnapshot from existing stores. Polls once per call;
// no subscriptions, no event tailing.
//
// This file is the thin composition layer: load (I/O) → project (pure).
// I/O lives in `snapshot-loader.ts`; derivation lives in `projection.ts`.
import type { MissionControlSnapshot } from "./types.js";
import {
  loadSnapshotInput,
  loadHomeSnapshotInput,
  type SnapshotDeps,
  type HomeSnapshotDeps,
  type SnapshotBuildOptions,
} from "./snapshot-loader.js";
import { projectSnapshot, projectHomeSnapshot } from "./projection.js";

export type { SnapshotDeps, HomeSnapshotDeps, SnapshotBuildOptions };

export {
  buildAgentGrid,
  buildDispatchQueue,
  buildEventStream,
  buildTimelineMilestones,
} from "./projection.js";
export { buildTaskBoard } from "./task-board.js";
export {
  buildPrincipleEffectivenessRows,
  buildReplyInbox,
} from "./reply-projection.js";

/**
 * Build a complete snapshot for the mission control dashboard.
 * Throws if mission not found.
 */
export async function buildSnapshot(
  deps: SnapshotDeps,
  missionId: string,
  options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const input = await loadSnapshotInput(deps, missionId, options);
  return projectSnapshot(input);
}

export async function buildHomeSnapshot(
  deps: HomeSnapshotDeps,
  options: SnapshotBuildOptions = {},
): Promise<MissionControlSnapshot> {
  const input = await loadHomeSnapshotInput(deps, options);
  return projectHomeSnapshot(input);
}
