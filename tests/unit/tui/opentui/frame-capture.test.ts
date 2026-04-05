import { describe, expect, it } from "bun:test";

import { renderOpenTuiPreviewFrame } from "../../../../src/tui/opentui/app/preview.js";
import { OPEN_TUI_THEME, resolveMissionControlTheme } from "../../../../src/tui/opentui/components/builders.js";
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

  it("resolves transparent chrome, opaque command palette, and solid detail modals for terminal background mode", () => {
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
      expect(theme.paletteModalBg).toBe(OPEN_TUI_THEME.panelBgElevated);
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

    it("renders the command palette with an opaque legacy surface and yellow selection in terminal mode", async () => {
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
      const selectedLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("navigate")));

      expect(titleLine).toBeDefined();
      expect(selectedLine).toBeDefined();

      const paletteTitleSpan = titleLine!.spans.find((span) => span.text.includes("Command Palette"));
      const paletteEscapeSpan = titleLine!.spans.find((span) => span.text.includes("esc"));
      const selectedSectionSpan = selectedLine!.spans.find((span) => span.text.includes("navigate"));
      const selectedLabelSpan = selectedLine!.spans.find((span) => span.text.includes("tasks"));
      const selectedHintSpan = selectedLine!.spans.find((span) => span.text.includes("[F]"));

      expect(paletteTitleSpan).toBeDefined();
      expect(paletteEscapeSpan).toBeDefined();
      expect(selectedSectionSpan).toBeDefined();
      expect(selectedLabelSpan).toBeDefined();
      expect(selectedHintSpan).toBeDefined();

      expect(paletteTitleSpan!.bg.buffer[0]).toBeCloseTo(0.0902, 3);
      expect(paletteTitleSpan!.bg.buffer[1]).toBeCloseTo(0.1294, 3);
      expect(paletteTitleSpan!.bg.buffer[2]).toBeCloseTo(0.1765, 3);
      expect(paletteEscapeSpan!.fg.buffer[0]).toBeCloseTo(0.5608, 3);
      expect(paletteEscapeSpan!.fg.buffer[1]).toBeCloseTo(0.6353, 3);
      expect(paletteEscapeSpan!.fg.buffer[2]).toBeCloseTo(0.7176, 3);

      expect(selectedSectionSpan!.bg.buffer[0]).toBe(1);
      expect(selectedSectionSpan!.bg.buffer[1]).toBeCloseTo(0.8196, 3);
      expect(selectedSectionSpan!.bg.buffer[2]).toBeCloseTo(0.4, 3);
      expect(selectedLabelSpan!.fg.buffer[0]).toBeCloseTo(0.0549, 3);
      expect(selectedLabelSpan!.fg.buffer[1]).toBeCloseTo(0.0823, 3);
      expect(selectedLabelSpan!.fg.buffer[2]).toBeCloseTo(0.1137, 3);
      expect(selectedHintSpan!.fg.buffer[0]).toBeCloseTo(0.0549, 3);
      expect(selectedHintSpan!.fg.buffer[1]).toBeCloseTo(0.0823, 3);
      expect(selectedHintSpan!.fg.buffer[2]).toBeCloseTo(0.1137, 3);
      expect(render.charFrame).toContain("> █");
      expect(render.charFrame).toContain("navigate      tasks");
      expect(render.charFrame).toContain("[F]");
    });
  });
