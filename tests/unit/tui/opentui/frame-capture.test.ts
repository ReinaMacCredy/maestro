import { describe, expect, it } from "bun:test";

import { renderOpenTuiPreviewFrame } from "@/tui/opentui/app/preview.js";
import { resolveMissionControlTheme } from "@/tui/opentui/components/builders.js";
import { buildModalModel, computeScreenLayout, getModalParentRect } from "@/tui/opentui/components/builders.js";
import { captureMissionControlFrame, captureMissionControlRender } from "@/tui/opentui/testing/frame-capture.js";
import { createInitialState, reduce } from "@/tui/state/reducer.js";
import type { MissionControlSnapshot } from "@/tui/state/types.js";
import { TextAttributes } from "@opentui/core";
import { layoutModal } from "@/tui/shared/modal-model.js";

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
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    configInspector: null,
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

  it("resolves transparent chrome, transparent command palette, and solid direct detail modals for terminal background mode", () => {
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
      session: null,
      pendingHandoffs: [],
      configSummary: {
        configSource: "global",
        gitAvailable: true,
        checks: [],
        missionDirectory: null,
        agentTypes: [],
        backgroundMode: "terminal",
      },
      configInspector: null,
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
        agentType: "test-skill",
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

    it("renders the command palette with a transparent legacy surface and yellow selection in terminal mode", async () => {
      const snapshot: MissionControlSnapshot = {
        ...makeSnapshot(),
        configSummary: {
          configSource: "global",
          gitAvailable: true,
          checks: [],
          missionDirectory: null,
          agentTypes: [],
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

      expect(paletteTitleSpan!.bg.buffer[0]).toBe(0);
      expect(paletteTitleSpan!.bg.buffer[1]).toBe(0);
      expect(paletteTitleSpan!.bg.buffer[2]).toBe(0);
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

  it("keeps underlying dashboard text visible through blank palette rows in terminal mode", async () => {
      const snapshot: MissionControlSnapshot = {
        ...makeSnapshot(),
        configSummary: {
          configSource: "global",
          gitAvailable: true,
          checks: [],
          missionDirectory: null,
          agentTypes: [],
          backgroundMode: "terminal",
        },
      };
      const baseState = createInitialState(snapshot);
      // Filter the palette down to a single command so the modal has
      // interior rows left blank below the result. An unfiltered palette
      // with all commands fills the modal completely with no blank rows.
      const frameWidth = 120;
      const frameHeight = 40;
      const paletteOpened = reduce(createInitialState(snapshot), { type: "open-command-palette" });
      const paletteState = "exit".split("").reduce(
        (acc, char) => reduce(acc, { type: "modal-query-append", char }),
        paletteOpened,
      );
      const baseRender = await captureMissionControlRender({
        snapshot,
        state: baseState,
        width: frameWidth,
        height: frameHeight,
      });
      const paletteRender = await captureMissionControlRender({
        snapshot,
        state: paletteState,
        width: frameWidth,
        height: frameHeight,
      });
      const modal = buildModalModel(paletteState);

      expect(modal).toBeDefined();

      const screenLayout = computeScreenLayout(frameWidth, frameHeight, snapshot);
      const modalParentRect = getModalParentRect(screenLayout);
      const modalLayout = layoutModal(modalParentRect, modal!);
      const left = modalLayout.x + 2;
      const right = modalLayout.x + modalLayout.width - 2;

      // Find a row inside the modal where the dashboard text is visible
      // through the palette's transparent interior. The exact row depends
      // on how many commands the palette currently has, so search from the
      // bottom of the modal upward until we find a row where the palette
      // slice equals the base slice (proving transparency).
      const paletteLines = paletteRender.charFrame.split("\n");
      const baseLines = baseRender.charFrame.split("\n");
      let transparentRow: number | undefined;
      for (let row = modalLayout.y + modalLayout.height - 2; row > modalLayout.y + 1; row -= 1) {
        const baseSlice = (baseLines[row] ?? "").slice(left, right);
        const paletteSlice = (paletteLines[row] ?? "").slice(left, right);
        if (baseSlice.trim().length > 0 && paletteSlice === baseSlice) {
          transparentRow = row;
          break;
        }
      }
      expect(transparentRow).toBeDefined();
  });

  it("renders palette-launched split overlays without opaque panel backgrounds in terminal mode", async () => {
    const snapshot: MissionControlSnapshot = {
      ...makeSnapshot(),
      configSummary: {
        configSource: "global",
        gitAvailable: true,
        checks: [],
        missionDirectory: null,
        agentTypes: [],
        backgroundMode: "terminal",
      },
    };
    const configState = reduce(
      reduce(createInitialState(snapshot), { type: "open-command-palette" }),
      { type: "open-config" },
    );
    const configRender = await captureMissionControlRender({
      snapshot,
      state: configState,
      width: 120,
      height: 40,
    });
    const modal = buildModalModel(configState);

    expect(modal).toBeDefined();
    expect(modal?.mode).toBe("split");
    if (!modal || modal.mode === "palette") {
      throw new Error("Expected a split modal");
    }
    expect(modal.returnTarget).toBe("command-palette");

    const titleLine = configRender.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Config")));
    const listLine = configRender.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Results")));
    const detailLine = configRender.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Details")));

    expect(titleLine).toBeDefined();
    expect(listLine).toBeDefined();
    expect(detailLine).toBeDefined();

    const titleSpan = titleLine!.spans.find((span) => span.text.includes("Config"));
    const listSpan = listLine!.spans.find((span) => span.text.includes("Results"));
    const detailSpan = detailLine!.spans.find((span) => span.text.includes("Details"));

    expect(titleSpan).toBeDefined();
    expect(listSpan).toBeDefined();
    expect(detailSpan).toBeDefined();

    expect(titleSpan!.bg.buffer[0]).toBe(0);
    expect(titleSpan!.bg.buffer[1]).toBe(0);
    expect(titleSpan!.bg.buffer[2]).toBe(0);
    expect(listSpan!.bg.buffer[0]).toBe(0);
    expect(listSpan!.bg.buffer[1]).toBe(0);
    expect(listSpan!.bg.buffer[2]).toBe(0);
    expect(detailSpan!.bg.buffer[0]).toBe(0);
    expect(detailSpan!.bg.buffer[1]).toBe(0);
    expect(detailSpan!.bg.buffer[2]).toBe(0);
  });

  it("renders palette-launched split overlay selection with the legacy yellow palette colors", async () => {
    const snapshot: MissionControlSnapshot = {
      ...makeSnapshot(),
      configSummary: {
        configSource: "global",
        gitAvailable: true,
        checks: [],
        missionDirectory: null,
        agentTypes: [],
        backgroundMode: "terminal",
      },
        pendingHandoffs: [
          {
            id: "handoff-1",
            agent: "codex",
            message: "Investigate handoff",
            timestamp: "2026-04-15T00:00:00.000Z",
          },
        ],
      };
    const handoffState = reduce(
      reduce(createInitialState(snapshot), { type: "open-command-palette" }),
      { type: "open-handoffs" },
    );
    const render = await captureMissionControlRender({
      snapshot,
      state: handoffState,
      width: 120,
      height: 40,
    });

    const selectedLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("handoff-1")));
    expect(selectedLine).toBeDefined();

    const selectedLabelSpan = selectedLine!.spans.find((span) => span.text.includes("handoff-1"));
    expect(selectedLabelSpan).toBeDefined();

    expect(selectedLabelSpan!.bg.buffer[0]).toBe(1);
    expect(selectedLabelSpan!.bg.buffer[1]).toBeCloseTo(0.8196, 3);
    expect(selectedLabelSpan!.bg.buffer[2]).toBeCloseTo(0.4, 3);
    expect(selectedLabelSpan!.fg.buffer[0]).toBeCloseTo(0.0549, 3);
    expect(selectedLabelSpan!.fg.buffer[1]).toBeCloseTo(0.0823, 3);
    expect(selectedLabelSpan!.fg.buffer[2]).toBeCloseTo(0.1137, 3);
  });

    it("dims the underlying dashboard while the command palette is open", async () => {
      const snapshot = makeSnapshot();
      const render = await captureMissionControlRender({
        snapshot,
        state: reduce(createInitialState(snapshot), { type: "open-command-palette" }),
        width: 120,
        height: 40,
      });

      const headerLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("Mission Control")));
      const statusLine = render.spans.lines.find((line) => line.spans.some((span) => span.text.includes("RUNNING")));

      expect(headerLine).toBeDefined();
      expect(statusLine).toBeDefined();

      const missionControlSpan = headerLine!.spans.find((span) => span.text.includes("Mission Control"));
      const runningSpan = statusLine!.spans.find((span) => span.text.includes("RUNNING"));

      expect(missionControlSpan).toBeDefined();
      expect(runningSpan).toBeDefined();
      expect(missionControlSpan!.attributes & TextAttributes.DIM).toBe(TextAttributes.DIM);
      expect(runningSpan!.attributes & TextAttributes.DIM).toBe(TextAttributes.DIM);
      expect(missionControlSpan!.fg.buffer[0]).toBeLessThan(1);
      expect(runningSpan!.fg.buffer[0]).toBeLessThan(1);
    });
  });
