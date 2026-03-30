import { describe, expect, it } from "bun:test";
import { createInitialState, reduce, type AppState, type Action } from "../../../src/tui/state.js";
import type { MissionControlSnapshot } from "../../../src/tui/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
      mode: "mission",
      missionId: "2026-03-30-001",
    missionTitle: "Test",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 3, active: 0 },
    tokenCounters: null,
    activeFeature: {
      id: "f1",
      title: "Feature 1",
      status: "pending",
      milestoneId: "m1",
      milestoneTitle: "Milestone 1",
      workerType: "test",
      description: "Test",
      preconditions: undefined,
      expectedBehavior: undefined,
      verificationSteps: [],
      dependsOn: [],
      fulfills: [],
      validTransitions: ["assigned", "in-progress"],
    },
    features: [
      { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
      { id: "f2", title: "F2", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
      { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "test", hasReport: false },
    ],
    activeWorker: null,
    progressLog: [],
      milestones: [],
      canPause: true,
      canResume: false,
      home: null,
      ...overrides,
    };
}

function makeState(overrides?: Partial<AppState>): AppState {
  return {
    ...createInitialState(makeSnapshot()),
    ...overrides,
  };
}

describe("createInitialState", () => {
  it("sets default focus to features", () => {
    const state = createInitialState(makeSnapshot());
    expect(state.focusedPanel).toBe("features");
    expect(state.selectedFeatureIndex).toBe(0);
    expect(state.modal.kind).toBe("none");
    expect(state.running).toBe(true);
  });
});

describe("reduce", () => {
  describe("quit", () => {
    it("sets running to false", () => {
      const state = reduce(makeState(), { type: "quit" });
      expect(state.running).toBe(false);
    });
  });

  describe("navigate", () => {
    it("moves feature selection down", () => {
      const state = reduce(makeState(), { type: "navigate", direction: "down" });
      expect(state.selectedFeatureIndex).toBe(1);
    });

    it("moves feature selection up", () => {
      const s1 = reduce(makeState(), { type: "navigate", direction: "down" });
      const s2 = reduce(s1, { type: "navigate", direction: "up" });
      expect(s2.selectedFeatureIndex).toBe(0);
    });

    it("clamps at bounds", () => {
      const state = reduce(makeState(), { type: "navigate", direction: "up" });
      expect(state.selectedFeatureIndex).toBe(0);
    });

    it("navigates modal options when modal is open", () => {
      const state = makeState({
        modal: { kind: "feature-action", featureIndex: 0, selectedOption: 0 },
      });
      const next = reduce(state, { type: "navigate", direction: "down" });
      if (next.modal.kind === "feature-action") {
        expect(next.modal.selectedOption).toBe(1);
      }
    });
  });

  describe("focus", () => {
    it("switches focused panel", () => {
      const state = reduce(makeState(), { type: "focus", panel: "log" });
      expect(state.focusedPanel).toBe("log");
    });

    it("does not change focus when modal is open", () => {
      const state = makeState({
        modal: { kind: "directory" },
      });
      const next = reduce(state, { type: "focus", panel: "log" });
      expect(next.focusedPanel).toBe("features");
    });
  });

    describe("enter", () => {
      it("opens feature action modal when features focused", () => {
        const state = reduce(makeState(), { type: "enter" });
        expect(state.modal.kind).toBe("feature-action");
      });

      it("does nothing in home mode", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            mode: "home",
            home: {
              headline: "No project detected",
              summary: "Open a repo",
              locationLabel: "Outside a git repository",
              checks: [],
              actions: [],
              pendingHandoffs: [],
            },
          }),
        });
        const next = reduce(state, { type: "enter" });
        expect(next.modal.kind).toBe("none");
      });

      it("does nothing when no features", () => {
        const state = makeState({
        snapshot: makeSnapshot({ features: [] }),
      });
      const next = reduce(state, { type: "enter" });
      expect(next.modal.kind).toBe("none");
    });
  });

  describe("escape", () => {
    it("closes modal", () => {
      const state = makeState({
        modal: { kind: "directory" },
      });
      const next = reduce(state, { type: "escape" });
      expect(next.modal.kind).toBe("none");
    });

    it("unfocuses panel when no modal", () => {
      const state = reduce(makeState(), { type: "escape" });
      expect(state.focusedPanel).toBe("none");
    });
  });

  describe("open-dir", () => {
    it("opens directory modal", () => {
      const state = reduce(makeState(), { type: "open-dir" });
      expect(state.modal.kind).toBe("directory");
    });
  });

  describe("open-models", () => {
    it("opens models modal", () => {
      const state = reduce(makeState(), { type: "open-models" });
      expect(state.modal.kind).toBe("models");
    });
  });

  describe("update-snapshot", () => {
    it("updates snapshot and clamps selection", () => {
      const state = makeState({ selectedFeatureIndex: 5 });
      const newSnap = makeSnapshot({
        features: [
          { id: "f1", title: "F1", status: "done", milestoneId: "m1", workerType: "t", hasReport: false },
        ],
      });
      const next = reduce(state, { type: "update-snapshot", snapshot: newSnap });
      expect(next.snapshot.features.length).toBe(1);
      expect(next.selectedFeatureIndex).toBe(0);
    });
  });
});
