import type { MissionControlSnapshot } from "../state/types.js";
import type { ConfigPort } from "@/infra/ports/config.port.js";

export interface SnapshotBuildOptions {
  readonly includeTaskBoard?: boolean;
  readonly includeReplies?: boolean;
}

export interface SnapshotDeps {
  readonly missionStore?: unknown;
  readonly featureStore?: unknown;
  readonly config: ConfigPort;
}

export interface InteractiveOptions {
  snapshot: MissionControlSnapshot;
  snapshotDeps: SnapshotDeps;
  reloadSnapshot: (options?: SnapshotBuildOptions) => Promise<MissionControlSnapshot>;
}

// Phase 3 strip: Mission Control no longer tracks live agent runtimes,
// so the poll interval is simply the long interval. The function is
// retained as the only poll-cadence hook so future callers can slow or
// speed up polling without touching interactive.tsx directly.
export function getSnapshotPollIntervalMs(_snapshot: MissionControlSnapshot): number {
  return 5_000;
}
