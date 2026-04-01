import { describe, expect, it } from "bun:test";

import { keyToAction } from "../../../src/tui/app/input-dispatch.js";
import { createInitialState } from "../../../src/tui/state/reducer.js";
import type { MissionControlSnapshot } from "../../../src/tui/state/types.js";

const SNAPSHOT: MissionControlSnapshot = {
  mode: "mission",
  missionId: "mission-1",
  missionTitle: "Mission 1",
  missionStatus: "executing",
  effectiveStatus: "executing",
  elapsedMs: 0,
  featureProgress: { done: 0, total: 1, active: 1 },
  statusProgress: {
    completed: 0,
    total: 1,
    inFlight: 1,
    blocked: 0,
    queued: 0,
    completionPct: 0,
  },
  tokenCounters: null,
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
  milestones: [],
  activeFeature: null,
  activeWorker: null,
  session: null,
  pendingHandoffs: [],
  configSummary: null,
  runtimeProcesses: [],
  progressLog: [],
  canPause: false,
  canResume: false,
  home: null,
};

describe("keyToAction", () => {
  it("does not map Left Arrow on the command palette home view", () => {
    const state = createInitialState(SNAPSHOT);
    state.modal = { kind: "command-palette", query: "", selectedCommandIndex: 0 };

    const action = keyToAction({ type: "arrow", direction: "left" }, state);

    expect(action).toBeUndefined();
  });

  it("maps Left Arrow to back when a palette-launched detail overlay is open", () => {
    const state = createInitialState(SNAPSHOT);
    state.modal = { kind: "handoffs", selectedHandoffIndex: 0, returnTarget: "command-palette" };

    const action = keyToAction({ type: "arrow", direction: "left" }, state);

    expect(action).toEqual({ type: "navigate", direction: "left" });
  });

  it("maps Ctrl+Y to copy mode toggle", () => {
    const action = keyToAction({ type: "ctrl", char: "y" }, createInitialState(SNAPSHOT));

    expect(action).toEqual({ type: "toggle-copy-mode" });
  });

  it("does not map Left Arrow when the command palette is closed", () => {
    const action = keyToAction(
      { type: "arrow", direction: "left" },
      createInitialState(SNAPSHOT),
    );

    expect(action).toBeUndefined();
  });
});
