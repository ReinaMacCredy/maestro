import { describe, expect, it } from "bun:test";

import { renderOpenTuiPreviewFrame } from "../../../../src/tui/opentui/app/preview.js";
import { resolveMissionControlTheme } from "../../../../src/tui/opentui/components/builders.js";
import { captureMissionControlFrame, captureMissionControlRender } from "../../../../src/tui/opentui/testing/frame-capture.js";
import { createInitialState, reduce } from "../../../../src/tui/state/reducer.js";
import type { MissionControlSnapshot } from "../../../../src/tui/state/types.js";

function makeSnapshot(): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-04-04-001",
    missionTitle: "OpenTUI Scaffold",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 12_000,
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
    missionOverview: null,
    activeFeature: null,
    features: [],
    taskPreviews: [],
    activeWorker: null,
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    configInspector: null,
    workerHealth: [],
    runtimeProcesses: [],
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    home: null,
  };
}

describe("captureMissionControlFrame", () => {
  it("renders the OpenTUI Mission Control dashboard at operator size", async () => {
    const frame = await captureMissionControlFrame({
      snapshot: makeSnapshot(),
      width: 120,
      height: 40,
    });

    expect(frame).toContain("Mission Control");
    expect(frame).toContain("OpenTUI Scaffold");
    expect(frame).toContain("Mission Overview unavailable");
    expect(frame).toContain("Tasks");
  });

  it("renders a terminal-too-small fallback", async () => {
    const frame = await captureMissionControlFrame({
      snapshot: makeSnapshot(),
      width: 60,
      height: 8,
    });

    expect(frame).toContain("Mission Control");
    expect(frame).toContain("Terminal too small");
  });

  it("resolves transparent chrome, transparent command palette, and solid detail modals for terminal background mode", () => {
    const terminalSnapshot: MissionControlSnapshot = {
      mode: "mission",
      missionId: "2026-04-04-001",
      missionTitle: "OpenTUI Scaffold",
      missionStatus: "executing",
      effectiveStatus: "executing",
      elapsedMs: 12_000,
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
      missionOverview: null,
      activeFeature: null,
      features: [],
      taskPreviews: [],
      activeWorker: null,
      session: null,
      pendingHandoffs: [],
      configSummary: {
        configSource: "global",
        cassAvailable: true,
        gitAvailable: true,
        checks: [],
        missionDirectory: null,
        workerTypes: [],
        backgroundMode: "terminal",
      },
      configInspector: null,
      workerHealth: [],
      runtimeProcesses: [],
      progressLog: [],
      milestones: [],
      canPause: true,
      canResume: false,
      home: null,
    };
    const theme = resolveMissionControlTheme(terminalSnapshot);

      expect(theme.pageBg).toBeUndefined();
      expect(theme.panelBg).toBeUndefined();
      expect(theme.headerBg).toBeUndefined();
      expect(theme.paletteModalBg).toBeUndefined();
      expect(theme.modalBg).toBeTruthy();
      expect(theme.modalPanelBg).toBeTruthy();
      expect(theme.paletteSelectionBg).toBe("#ffd166");
      expect(theme.paletteSelectionFg).toBe("#0e151d");
    });

  it("sanitizes terminal control sequences in plain and ansi previews", async () => {
    const snapshot = makeSnapshot();
    snapshot.features = [
      {
        id: "f1",
        title: "Injected \u001b]2;PWN\u0007 Title",
        status: "in-progress",
        milestoneId: "m1",
        workerType: "test-skill",
        hasReport: false,
      },
    ];

    const plainFrame = await renderOpenTuiPreviewFrame({
      snapshot,
      screen: "features",
      width: 120,
      height: 40,
      format: "plain",
    });
    const ansiFrame = await renderOpenTuiPreviewFrame({
      snapshot,
      screen: "features",
      width: 120,
      height: 40,
      format: "ansi",
    });

    expect(plainFrame).toContain("Injected  Title");
      expect(plainFrame).not.toContain("\u001b]2;PWN\u0007");
      expect(ansiFrame).not.toContain("]2;PWN");
    });

    it("renders the command palette with a transparent surface and legacy yellow selection in terminal mode", async () => {
      const snapshot: MissionControlSnapshot = {
        ...makeSnapshot(),
        configSummary: {
          configSource: "global",
          cassAvailable: true,
          gitAvailable: true,
          checks: [],
          missionDirectory: null,
          workerTypes: [],
          backgroundMode: "terminal",
        },
      };
      const state = reduce(createInitialState(snapshot), { type: "open-command-palette" });
      const render = await captureMissionControlRender({
        snapshot,
        state,
        width: 120,
        height: 40,
      });

      const titleLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Command Palette")));
      const selectedLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Tasks  Browse mission tasks")));

      expect(titleLine).toBeDefined();
      expect(selectedLine).toBeDefined();

      const paletteTitleSpan = titleLine!.spans.find((span) => span.text.includes("Command Palette"));
      const selectedCommandSpan = selectedLine!.spans.find((span) => span.text.includes("Tasks  Browse mission tasks"));

      expect(paletteTitleSpan).toBeDefined();
      expect(selectedCommandSpan).toBeDefined();

      expect(paletteTitleSpan!.bg.buffer[0]).toBe(0);
      expect(paletteTitleSpan!.bg.buffer[1]).toBe(0);
      expect(paletteTitleSpan!.bg.buffer[2]).toBe(0);

      expect(selectedCommandSpan!.bg.buffer[0]).toBe(1);
      expect(selectedCommandSpan!.bg.buffer[1]).toBeCloseTo(0.8196, 3);
      expect(selectedCommandSpan!.bg.buffer[2]).toBeCloseTo(0.4, 3);
      expect(selectedCommandSpan!.fg.buffer[0]).toBeCloseTo(0.0549, 3);
      expect(selectedCommandSpan!.fg.buffer[1]).toBeCloseTo(0.0823, 3);
      expect(selectedCommandSpan!.fg.buffer[2]).toBeCloseTo(0.1137, 3);
    });
  });
