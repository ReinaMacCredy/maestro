import { describe, expect, it } from "bun:test";
import { buildPreviewState, getApplicablePreviewScreens, PREVIEW_SCREENS } from "@/tui/app/preview-state.js";
import type { MissionControlSnapshot } from "@/tui/state/types.js";

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
      agentType: "test-skill",
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
        agentType: "test-skill",
        hasReport: false,
      },
      {
        id: "f2",
        title: "Feature Two",
        status: "pending",
        milestoneId: "m1",
        agentType: "test-skill",
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
        agentType: "test-skill",
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
        agentType: "test-skill",
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
    session: null,
    configSummary: {
      configSource: "project",
      gitAvailable: true,
      checks: [],
      missionDirectory: ".maestro/missions/2026-04-02-001",
      backgroundMode: "solid",
    },
    progressLog: [],
    milestones: [{ id: "m1", title: "Setup", status: "executing", order: 0 }],
    canPause: true,
    canResume: false,
    home: null,
    ...overrides,
  };
}

describe("buildPreviewState", () => {
  it("PREVIEW_SCREENS includes autopilot and has 14 entries", () => {
    expect(PREVIEW_SCREENS).toContain("autopilot");
    expect(PREVIEW_SCREENS).toHaveLength(14);
  });

  it("getApplicablePreviewScreens returns autopilot for mission mode", () => {
    const screens = getApplicablePreviewScreens({ mode: "mission" });
    expect(screens).toContain("autopilot");
  });

  it("keeps home previews limited to non-mission screens (no autopilot)", () => {
    const screens = getApplicablePreviewScreens({ mode: "home" });

    expect(screens).toEqual([
      "dashboard",
      "features",
      "config",
      "memory",
      "graph",
      "agents",
      "events",
      "tasks",
      "principles",
      "help",
    ]);
    expect(screens).not.toContain("autopilot");
  });

  it("defaults to the overview left pane when no selector is provided", () => {
    const state = buildPreviewState({ snapshot: makeSnapshot() });

    // Phase 3 strip: the live-feature auto-follow path was deleted.
    // The preview now starts in overview mode on the first feature.
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

  // Phase 3 strip: the runtime, agents, and output previews were
  // deleted. Their data was removed with the agent execution layer
  // in Phase 1 and the screens are removed from the preview set in
  // Commit 3.2.

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
        screen: "config",
        featureId: "f1",
      })
      ).toThrow("--feature is only supported");
  });

  it("opens autopilot screen in mission mode", () => {
    const state = buildPreviewState({
      snapshot: makeSnapshot(),
      screen: "autopilot",
    });

    expect(state.modal).toEqual({
      kind: "autopilot",
      selectedIndex: 0,
      returnTarget: undefined,
    });
  });

  it("rejects autopilot preview in home mode", () => {
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
          },
        }),
        screen: "autopilot",
      })
    ).toThrow("Autopilot preview requires a mission");
  });
});
