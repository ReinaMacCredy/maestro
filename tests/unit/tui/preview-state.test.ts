import { describe, expect, it } from "bun:test";
import { buildPreviewState } from "../../../src/tui/app/preview-state.js";
import type { MissionControlSnapshot } from "../../../src/tui/state/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-04-02-001",
    missionTitle: "Preview Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 42_000,
    featureProgress: { done: 0, total: 2, active: 1 },
    statusProgress: {
      completed: 0,
      total: 2,
      inFlight: 1,
      blocked: 0,
      queued: 1,
      completionPct: 0,
    },
    tokenCounters: null,
    missionOverview: {
      missionLabel: "Mission: Preview Test Mission",
      statusLabel: "executing",
      activeCount: 1,
      doneCount: 0,
      totalCount: 2,
      blockedCount: 0,
      currentMilestone: "Setup",
      gateLabel: null,
      agentSummary: [{ agent: "codex", count: 1 }],
      dependencyMap: [],
    },
    activeFeature: {
      id: "f1",
      title: "Feature One",
      status: "assigned",
      milestoneId: "m1",
      milestoneTitle: "Setup",
      workerType: "test-skill",
      description: "First feature",
      preconditions: undefined,
      expectedBehavior: undefined,
      verificationSteps: [],
      dependsOn: [],
      blockedBy: [],
      unblocks: [],
      fulfills: [],
      validTransitions: ["in-progress"],
    },
    features: [
      {
        id: "f1",
        title: "Feature One",
        status: "assigned",
        milestoneId: "m1",
        workerType: "test-skill",
        hasReport: false,
      },
      {
        id: "f2",
        title: "Feature Two",
        status: "pending",
        milestoneId: "m1",
        workerType: "test-skill",
        hasReport: false,
        blockedByIds: ["f1"],
      },
    ],
    taskPreviews: [
      {
        id: "f1",
        title: "Feature One",
        status: "assigned",
        milestoneId: "m1",
        milestoneTitle: "Setup",
        workerType: "test-skill",
        description: "First feature",
        preconditions: undefined,
        expectedBehavior: undefined,
        verificationSteps: [],
        dependsOn: [],
        blockedBy: [],
        unblocks: [{ id: "f2", title: "Feature Two", status: "pending" }],
        fulfills: [],
        validTransitions: ["in-progress"],
      },
      {
        id: "f2",
        title: "Feature Two",
        status: "pending",
        milestoneId: "m1",
        milestoneTitle: "Setup",
        workerType: "test-skill",
        description: "Second feature",
        preconditions: undefined,
        expectedBehavior: undefined,
        verificationSteps: [],
        dependsOn: ["f1"],
        blockedBy: [{ id: "f1", title: "Feature One", status: "assigned" }],
        unblocks: [],
        fulfills: [],
        validTransitions: ["assigned"],
      },
    ],
    activeWorker: null,
    session: null,
    pendingHandoffs: [
      { id: "handoff-1", agent: "codex", message: "First handoff" },
      { id: "handoff-2", agent: "claude", message: "Second handoff" },
    ],
    configSummary: {
      configSource: "project",
      cassAvailable: true,
      gitAvailable: true,
      checks: [],
      missionDirectory: ".maestro/missions/2026-04-02-001",
      workerTypes: ["test-skill"],
    },
    runtimeProcesses: [
      {
        featureId: "f1",
        title: "Feature One",
        status: "assigned",
        workerType: "test-skill",
        hasReport: false,
        isLive: true,
      },
    ],
    progressLog: [],
    milestones: [{ id: "m1", title: "Setup", status: "executing", order: 0 }],
    canPause: true,
    canResume: false,
    home: null,
    ...overrides,
  };
}

describe("buildPreviewState", () => {
  it("defaults to the dashboard preview", () => {
    const state = buildPreviewState({ snapshot: makeSnapshot() });

    expect(state.modal.kind).toBe("none");
    expect(state.leftPaneMode).toBe("overview");
    expect(state.selectedFeatureIndex).toBe(0);
  });

  it("shows the requested feature on dashboard previews", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "dashboard",
      featureId: "f2",
    });

    expect(state.modal.kind).toBe("none");
    expect(state.leftPaneMode).toBe("preview");
    expect(state.selectedFeatureIndex).toBe(1);
  });

  it("opens the mission feature browser for features previews", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "features",
    });

    expect(state.modal).toEqual({
      kind: "feature-browser",
      selectedFeatureIndex: 0,
      returnTarget: undefined,
    });
  });

  it("opens the overview modal for features previews in home mode", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot({
        mode: "home",
        features: [],
        taskPreviews: [],
        activeFeature: null,
        home: {
          headline: "No missions yet",
          summary: "Create your first mission.",
          locationLabel: "In a git repository",
          checks: [],
          actions: [],
          pendingHandoffs: [],
        },
      }),
      screen: "features",
    });

    expect(state.modal).toEqual({ kind: "overview", returnTarget: undefined });
  });

  it("opens dependencies for the requested feature", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "dependencies",
      featureId: "f2",
    });

    expect(state.selectedFeatureIndex).toBe(1);
    expect(state.modal).toEqual({
      kind: "dependencies",
      selectedOption: 0,
      returnTarget: undefined,
    });
  });

  it("opens the requested handoff in the handoffs modal", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "handoffs",
      handoffId: "handoff-2",
    });

    expect(state.modal).toEqual({
      kind: "handoffs",
      selectedHandoffIndex: 1,
      returnTarget: undefined,
    });
  });

    it("opens the config screen", () => {
      const state = buildPreviewState({
        snapshot: makeSnapshot(),
        screen: "config",
      });

      expect(state.modal).toEqual({
        kind: "config",
        tab: "overview",
        selectedRowIndex: 0,
        phase: "browse",
        selectedScope: "project",
        returnTarget: undefined,
      });
    });

  it("opens the runtime screen", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "runtime",
    });

    expect(state.modal).toEqual({
      kind: "processes",
      selectedProcessIndex: 0,
      returnTarget: undefined,
    });
  });

  it("rejects dependencies previews in home mode", () => {
    expect(() =>
      buildPreviewState({
        snapshot: makeSnapshot({
          mode: "home",
          features: [],
          taskPreviews: [],
          activeFeature: null,
          home: {
            headline: "No missions yet",
            summary: "Create your first mission.",
            locationLabel: "In a git repository",
            checks: [],
            actions: [],
            pendingHandoffs: [],
          },
        }),
        screen: "dependencies",
      })
    ).toThrow("Dependencies preview requires a mission");
  });

  it("rejects feature selectors on unsupported screens", () => {
    expect(() =>
      buildPreviewState({
        snapshot: makeSnapshot(),
        screen: "handoffs",
        featureId: "f1",
      })
    ).toThrow("--feature is only supported");
  });

  it("rejects unknown handoff selectors", () => {
    expect(() =>
      buildPreviewState({
        snapshot: makeSnapshot(),
        screen: "handoffs",
        handoffId: "handoff-missing",
      })
    ).toThrow("Handoff handoff-missing not found");
  });
});
