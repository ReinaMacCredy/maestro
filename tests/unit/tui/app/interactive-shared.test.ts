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
    activeFeature: null,
    features: [],
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    home: null,
    ...overrides,
  };
}

describe("getSnapshotPollIntervalMs", () => {
  // Phase 3 strip: the "faster polling when a runtime is active" path
  // read `snapshot.runtimeProcesses` which was deleted with the worker
  // execution layer. The TUI now polls at a single cadence.
  it("returns the default polling interval", () => {
    const interval = getSnapshotPollIntervalMs(makeSnapshot());

    expect(interval).toBe(5_000);
  });
});
