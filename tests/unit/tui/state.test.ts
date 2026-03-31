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
    statusProgress: {
      completed: 0,
      total: 3,
      inFlight: 0,
      blocked: 0,
      queued: 3,
      completionPct: 0,
    },
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
      session: {
        branch: "main",
        workingTreeClean: false,
        diffStat: "+4 -1",
        changedFiles: ["src/tui/index.ts"],
      },
      pendingHandoffs: [],
      configSummary: {
        configSource: "project",
        cassAvailable: true,
        gitAvailable: true,
        checks: [],
        missionDirectory: ".maestro/missions/2026-03-30-001",
        workerTypes: ["test"],
      },
      runtimeProcesses: [],
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
        modal: { kind: "feature-action", featureIndex: 0, selectedOption: 0, phase: "selecting" },
      });
      const next = reduce(state, { type: "navigate", direction: "down" });
      if (next.modal.kind === "feature-action") {
        expect(next.modal.selectedOption).toBe(1);
        expect(next.modal.phase).toBe("selecting");
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
          modal: { kind: "config" },
        });
        const next = reduce(state, { type: "focus", panel: "log" });
        expect(next.focusedPanel).toBe("features");
    });
  });

    describe("enter", () => {
      it("opens feature action modal when features focused", () => {
        const state = reduce(makeState(), { type: "enter" });
        expect(state.modal.kind).toBe("feature-action");
        if (state.modal.kind === "feature-action") {
          expect(state.modal.phase).toBe("selecting");
        }
      });

      it("moves feature action modal into confirming state on enter", () => {
        const opened = reduce(makeState(), { type: "enter" });
        const confirmed = reduce(opened, { type: "enter" });
        expect(confirmed.modal.kind).toBe("feature-action");
        if (confirmed.modal.kind === "feature-action") {
          expect(confirmed.modal.phase).toBe("confirming");
        }
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
          modal: { kind: "config" },
        });
        const next = reduce(state, { type: "escape" });
        expect(next.modal.kind).toBe("none");
    });

    it("unfocuses panel when no modal", () => {
      const state = reduce(makeState(), { type: "escape" });
      expect(state.focusedPanel).toBe("none");
    });
  });

  describe("open-features", () => {
    it("opens feature browser", () => {
      const state = reduce(makeState(), { type: "open-features" });
      expect(state.modal.kind).toBe("feature-browser");
    });

    it("opens the feature browser from the command palette", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "fea", selectedCommandIndex: 1 } }),
        { type: "open-features" },
      );
      expect(state.modal.kind).toBe("feature-browser");
    });
  });

  describe("open-handoffs", () => {
    it("opens handoffs modal", () => {
      const state = reduce(makeState(), { type: "open-handoffs" });
      expect(state.modal.kind).toBe("handoffs");
    });
  });

  describe("open-config", () => {
    it("opens config modal", () => {
      const state = reduce(makeState(), { type: "open-config" });
      expect(state.modal.kind).toBe("config");
    });
  });

  describe("open-processes", () => {
    it("opens processes modal", () => {
      const state = reduce(makeState(), { type: "open-processes" });
      expect(state.modal.kind).toBe("processes");
    });
  });

  describe("command palette", () => {
    it("opens the command palette with a fresh query", () => {
      const state = reduce(makeState(), { type: "open-command-palette" });
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("appends query characters and resets selection", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "", selectedCommandIndex: 3 } }),
        { type: "modal-query-append", char: "p" },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("p");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("backspaces the query and resets selection", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "proc", selectedCommandIndex: 2 } }),
        { type: "modal-query-backspace" },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("pro");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("updates the selected command index", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "", selectedCommandIndex: 0 } }),
        { type: "modal-select", option: 2 },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.selectedCommandIndex).toBe(2);
      }
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

    it("preserves the selected feature by id when the snapshot order changes", () => {
      const state = makeState({ selectedFeatureIndex: 1 });
      const reordered = makeSnapshot({
        features: [
          { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "t", hasReport: false },
          { id: "f2", title: "F2", status: "assigned", milestoneId: "m1", workerType: "t", hasReport: false },
          { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "t", hasReport: false },
        ],
      });
      const next = reduce(state, { type: "update-snapshot", snapshot: reordered });
      expect(next.selectedFeatureIndex).toBe(1);
      expect(next.snapshot.features[next.selectedFeatureIndex]?.id).toBe("f2");
    });
  });
});
