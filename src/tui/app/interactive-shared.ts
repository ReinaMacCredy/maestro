import type { SnapshotDeps } from "../state/snapshot.js";
import type { MissionControlSnapshot } from "../state/types.js";

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  reloadSnapshot: () => Promise<MissionControlSnapshot>;
}

export function getSnapshotPollIntervalMs(snapshot: MissionControlSnapshot): number {
  const hasActiveRuntime = snapshot.runtimeProcesses.some((process) =>
    process.isLive
    || process.runtimeState === "starting"
    || process.runtimeState === "stale"
    || process.runtimeState === "recoverable"
  );
  return hasActiveRuntime ? 1_000 : 5_000;
}
