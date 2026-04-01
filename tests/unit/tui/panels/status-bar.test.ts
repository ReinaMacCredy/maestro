import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderStatusBar } from "../../../../src/tui/panels/status-bar.js";
import type { MissionControlSnapshot } from "../../../../src/tui/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 754_000,
    featureProgress: { done: 2, total: 4, active: 1 },
    statusProgress: {
      completed: 2,
      total: 4,
      inFlight: 1,
      blocked: 1,
      queued: 0,
      completionPct: 50,
    },
    tokenCounters: null,
    activeFeature: null,
    features: [],
    activeWorker: null,
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    home: null,
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    runtimeProcesses: [],
    ...overrides,
  };
}

describe("renderStatusBar", () => {
  it("shows RUNNING label for executing status", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("RUNNING");
  });

  it("shows progress counts", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("2/4 done");
  });

  it("shows active count when > 0", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("1 active");
  });

  it("shows blocked count when present", () => {
    const buf = new Buffer(90, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 90, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("1 blocked");
  });

  it("keeps done counts visible first at narrow widths", () => {
    const buf = new Buffer(46, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 46, height: 1 }, makeSnapshot({
      statusProgress: {
        completed: 2,
        total: 4,
        inFlight: 1,
        blocked: 1,
        queued: 3,
        completionPct: 50,
      },
    }));
    const text = buf.toString();
    expect(text).toContain("2/4 done");
  });

  it("fills the rail from completion only, not active work", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot({
      statusProgress: {
        completed: 1,
        total: 4,
        inFlight: 2,
        blocked: 0,
        queued: 1,
        completionPct: 25,
      },
      featureProgress: { done: 1, total: 4, active: 2 },
    }));

    const filledCells = Array.from({ length: buf.width }, (_, x) => buf.getCell(0, x))
      .filter((cell) => cell?.bg === 208).length;

    expect(filledCells).toBeLessThan(20);
  });

  it("shows filled circle dot", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("\u25cf"); // ●
  });

  it("shows a home banner for home-mode snapshots", () => {
    const buf = new Buffer(100, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 100, height: 1 }, makeSnapshot({
      mode: "home",
      home: {
        headline: "No missions yet",
        summary: "Create your first mission",
        locationLabel: process.cwd(),
        checks: [{ name: "git", status: "ok", message: "Git repository detected" }],
        actions: [{ label: "Initialize", command: "maestro init", detail: "Set up the project" }],
        pendingHandoffs: [],
      },
    }));
    const text = buf.toString();
    expect(text).toContain("HOME");
    expect(text).toContain("No missions yet");
  });

  it("shows active milestone profile and gate blocked detail", () => {
    const buf = new Buffer(120, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 120, height: 1 }, makeSnapshot({
      milestones: [
        { id: "m1", title: "Plan Review", status: "executing", order: 0, kind: "gate", profile: "plan-review" },
      ],
      gateBlocked: true,
      gateLabel: "Plan Review",
    }));

    const text = buf.toString();
    expect(text).toContain("PLAN-REVIEW");
    expect(text).toContain("Plan Review BLOCKED");
  });

  it("preserves spacing between milestone profile and title at narrow widths", () => {
    const buf = new Buffer(60, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 60, height: 1 }, makeSnapshot({
      milestones: [
        { id: "m1", title: "Foundation", status: "executing", order: 0, kind: "work", profile: "custom" },
      ],
      statusProgress: {
        completed: 1,
        total: 3,
        inFlight: 2,
        blocked: 0,
        queued: 0,
        completionPct: 33,
      },
      featureProgress: { done: 1, total: 3, active: 2 },
    }));

    const text = buf.toString();
    expect(text).toContain("CUSTOM Foundation");
    expect(text).toContain("1/3 done");
  });
});
