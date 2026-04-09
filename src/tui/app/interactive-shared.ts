import type { SnapshotDeps } from "../state/snapshot.js";
import type { MissionControlSnapshot } from "../state/types.js";

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  reloadSnapshot: () => Promise<MissionControlSnapshot>;
}

// Phase 3 strip: Mission Control no longer tracks live worker runtimes,
// so the poll interval is simply the long interval. The function is
// retained as the only poll-cadence hook so future callers can slow or
// speed up polling without touching interactive.tsx directly.
export function getSnapshotPollIntervalMs(_snapshot: MissionControlSnapshot): number {
  return 5_000;
}
