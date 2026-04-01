import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderFooter } from "../../../../src/tui/panels/footer.js";
import { PALETTE } from "../../../../src/tui/theme.js";
import type { MissionControlSnapshot } from "../../../../src/tui/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 120_000,
    featureProgress: { done: 1, total: 3, active: 1 },
    statusProgress: {
      completed: 1,
      total: 3,
      inFlight: 1,
      blocked: 0,
      queued: 1,
      completionPct: 33,
    },
    tokenCounters: null,
    session: {
      branch: "main",
      workingTreeClean: false,
      diffStat: "+4 -1",
      changedFiles: ["src/tui/footer.ts"],
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
    activeFeature: null,
    features: [],
    activeWorker: null,
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    home: null,
    ...overrides,
  };
}

describe("renderFooter", () => {
  it("shows Tasks hint", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("F");
    expect(text).toContain("Tasks");
  });

  it("shows Dependencies, Handoffs, Config, Runtime, and Copy hints", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Dependencies");
    expect(text).toContain("Handoffs");
    expect(text).toContain("Config");
    expect(text).toContain("Runtime");
    expect(text).toContain("Copy");
  });

  it("shows Exit hint", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Ctrl+T");
    expect(text).toContain("Exit");
  });

  it("uses dimmer label text for footer labels than the key", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot());

    const keyCell = buf.getCell(0, 1);
    const featuresLabelCell = buf.getCell(0, 3);
    expect(keyCell?.char).toBe("F");
    expect(keyCell?.fg).toBe(PALETTE.brightWhite);
    expect(featuresLabelCell?.char).toBe("T");
    expect(featuresLabelCell?.fg).toBe(PALETTE.gray);
  });

  it("preserves Ctrl+T on narrow widths", () => {
    const buf = new Buffer(60, 1);
    renderFooter(buf, { x: 0, y: 0, width: 60, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Ctrl+T");
  });

  it("uses home-mode hints outside mission context", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot({
      mode: "home",
      home: {
        headline: "No project detected",
        summary: "Open a repo",
        locationLabel: "Outside a git repository",
        checks: [],
        actions: [],
        pendingHandoffs: [],
      },
    }));
    const text = buf.toString();
    expect(text).toContain("Overview");
    expect(text).toContain("Handoffs");
    expect(text).toContain("Config");
    expect(text).not.toContain("Dependencies");
    expect(text).not.toContain("Runtime");
  });

  it("shows a copy mode banner when copy mode is active", () => {
    const buf = new Buffer(120, 1);
    renderFooter(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot(), true);

    const text = buf.toString();
    expect(text).toContain("COPY MODE ACTIVE");
    expect(text).toContain("drag-select enabled");
  });
});
