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
  configInspector: {
    tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor"],
    rowsByTab: {
      overview: [],
        effective: [{
          keyPath: "execution.stopOnFailure",
          label: "Stop on failure",
          section: "Execution",
          valueText: "on",
          displayValueText: "on",
          source: "default",
          sourceBadge: "D",
          editKind: "toggle",
          editKindLabel: "on/off",
          options: ["off", "on"],
          description: "desc",
          summary: "Choose whether Maestro stops after the first failed task.",
          impactText: "If this is on, the run stops on the first failure.",
          effectiveValueText: "on",
          effectiveDisplayValueText: "on",
        }],
      project: [],
      global: [],
      defaults: [],
      workers: [],
      plan: [],
      doctor: [],
    },
    hasProjectConfig: true,
    hasGlobalConfig: true,
    projectPath: ".maestro/config.yaml",
    globalPath: "~/.maestro/config.yaml",
    errors: [],
  },
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

  it("maps slash to the command palette when no modal is open", () => {
    const action = keyToAction({ type: "char", char: "/" }, createInitialState(SNAPSHOT));

    expect(action).toEqual({ type: "open-command-palette" });
  });

  it("does not map Left Arrow when the command palette is closed", () => {
    const action = keyToAction(
      { type: "arrow", direction: "left" },
      createInitialState(SNAPSHOT),
    );

    expect(action).toBeUndefined();
  });

  it("cycles config values with arrow keys while editing", () => {
    const state = createInitialState(SNAPSHOT);
    state.modal = {
      kind: "config",
      tab: "effective",
      selectedRowIndex: 0,
      phase: "edit-inline",
      selectedScope: "project",
      draftValue: "on",
    };

    const action = keyToAction({ type: "arrow", direction: "right" }, state);

    expect(action).toEqual({ type: "config-cycle-value", direction: "next" });
  });

  it("maps Left Arrow to back when config was opened from the command palette", () => {
    const state = createInitialState(SNAPSHOT);
    state.modal = {
      kind: "config",
      tab: "overview",
      selectedRowIndex: 0,
      phase: "browse",
      selectedScope: "project",
      returnTarget: "command-palette",
      returnPalette: { query: "conf", selectedCommandIndex: 0 },
    };

    const action = keyToAction({ type: "arrow", direction: "left" }, state);

    expect(action).toEqual({ type: "navigate", direction: "left" });
  });

    it("switches config tabs with bracket hotkeys", () => {
      const state = createInitialState(SNAPSHOT);
      state.modal = {
        kind: "config",
      tab: "overview",
      selectedRowIndex: 0,
      phase: "browse",
      selectedScope: "project",
    };

      expect(keyToAction({ type: "char", char: "]" }, state)).toEqual({ type: "config-next-tab" });
      expect(keyToAction({ type: "char", char: "[" }, state)).toEqual({ type: "config-prev-tab" });
    });

    it("opens config row finder with slash", () => {
      const state = createInitialState(SNAPSHOT);
      state.modal = {
        kind: "config",
        tab: "overview",
        selectedRowIndex: 0,
        phase: "browse",
        selectedScope: "project",
      };

      expect(keyToAction({ type: "char", char: "/" }, state)).toEqual({ type: "config-find-start" });
    });

    it("routes characters into the config row finder when search is open", () => {
      const state = createInitialState(SNAPSHOT);
      state.modal = {
        kind: "config",
        tab: "overview",
        selectedRowIndex: 0,
        phase: "browse",
        selectedScope: "project",
        findQuery: "def",
      };

      expect(keyToAction({ type: "char", char: "a" }, state)).toEqual({ type: "config-find-append", char: "a" });
      expect(keyToAction({ type: "backspace" }, state)).toEqual({ type: "config-find-backspace" });
    });
  });
