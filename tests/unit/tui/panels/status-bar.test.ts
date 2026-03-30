import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderStatusBar } from "../../../../src/tui/panels/status-bar.js";
import type { MissionControlSnapshot } from "../../../../src/tui/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 754_000,
    featureProgress: { done: 2, total: 4, active: 1 },
    tokenCounters: null,
    activeFeature: null,
    features: [],
    activeWorker: null,
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
    ...overrides,
  };
}

describe("renderStatusBar", () => {
  it("shows status label in uppercase", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("EXECUTING");
  });

  it("shows progress counts", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("2/4");
  });

  it("shows active count when > 0", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("[+1]");
  });

  it("shows elapsed time", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Elapsed:");
    expect(text).toContain("12m");
  });

  it("renders progress bar", () => {
    const buf = new Buffer(80, 1);
    renderStatusBar(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("[");
    expect(text).toContain("]");
  });
});
