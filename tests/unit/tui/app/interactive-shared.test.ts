import { describe, expect, it } from "bun:test";
import { getSnapshotPollIntervalMs } from "../../../../src/tui/app/interactive-shared.js";
import type { MissionControlSnapshot } from "../../../../src/tui/state/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 120_000,
    featureProgress: { done: 1, total: 3, active: 1 },
    statusProgress: {
      completed: 1,
      total: 3,
      inFlight: 1,
      blocked: 0,
      queued: 1,
      completionPct: 33,
    },
    tokenCounters: null,
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    runtimeProcesses: [],
    activeFeature: null,
    features: [],
    activeWorker: null,
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    home: null,
    ...overrides,
  };
}

describe("getSnapshotPollIntervalMs", () => {
  it("uses faster polling when a runtime is active", () => {
    const interval = getSnapshotPollIntervalMs(makeSnapshot({
      runtimeProcesses: [{
        featureId: "f2",
        title: "Configure database",
        status: "in-progress",
        workerType: "backend-worker",
        hasReport: false,
        isLive: true,
      }],
    }));

    expect(interval).toBe(1_000);
  });

  it("keeps the default polling interval when there is no active runtime", () => {
    const interval = getSnapshotPollIntervalMs(makeSnapshot());

    expect(interval).toBe(5_000);
  });
});
