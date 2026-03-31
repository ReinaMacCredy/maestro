import { describe, expect, it } from "bun:test";

import { keyToAction } from "../../../src/tui/index.js";
import { createInitialState } from "../../../src/tui/state.js";
import type { MissionControlSnapshot } from "../../../src/tui/types.js";

const SNAPSHOT: MissionControlSnapshot = {
  mode: "mission",
  missionId: "mission-1",
  missionTitle: "Mission 1",
  missionPath: "/tmp/mission-1",
  effectiveStatus: "executing",
  headline: "Mission Control",
  summary: "Summary",
  featureProgress: {
    completed: 0,
    total: 1,
    active: 1,
    percent: 0,
  },
  statusProgress: {
    completed: 0,
    total: 1,
    inFlight: 1,
    blocked: 0,
    queued: 0,
    completionPct: 0,
  },
  features: [
    {
      id: "f1",
      title: "Feature 1",
      status: "assigned",
      milestoneId: "m1",
      workerType: "backend",
      hasReport: false,
    },
  ],
  activeFeature: undefined,
  activeWorker: undefined,
  progressLog: [],
  recentEvents: [],
  activity: {
    headline: "Task",
    meta: "f1",
    state: "Waiting",
    next: "Start work",
    scope: "Current feature",
  },
  session: {
    durationSeconds: 0,
    branch: "main",
    workingTreeClean: true,
    diffStat: "clean",
    changedFiles: [],
  },
  pendingHandoffs: [],
  configSummary: {
    hasGlobalConfig: true,
    hasProjectConfig: true,
    cassHealthy: true,
    workerModel: "gpt-5",
    reviewerModel: "gpt-5",
  },
  processRows: [],
  generatedAt: "2026-03-31T00:00:00.000Z",
};

describe("keyToAction", () => {
  it("maps Left Arrow to back when the command palette is open", () => {
    const state = createInitialState(SNAPSHOT);
    state.modal = { kind: "command-palette", query: "", selectedCommandIndex: 0 };

    const action = keyToAction({ type: "arrow", direction: "left" }, state);

    expect(action).toEqual({ type: "escape" });
  });

  it("does not map Left Arrow when the command palette is closed", () => {
    const action = keyToAction(
      { type: "arrow", direction: "left" },
      createInitialState(SNAPSHOT),
    );

    expect(action).toBeUndefined();
  });
});
