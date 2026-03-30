import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderHeader } from "../../../../src/tui/panels/header.js";
import type { MissionControlSnapshot } from "../../../../src/tui/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 120_000,
    featureProgress: { done: 1, total: 3, active: 1 },
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

describe("renderHeader", () => {
  it("renders mission title", () => {
    const buf = new Buffer(80, 1);
    renderHeader(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Mission Control");
    expect(text).toContain("Test Mission");
  });

  it("truncates long title to width", () => {
    const buf = new Buffer(30, 1);
    renderHeader(
      buf,
      { x: 0, y: 0, width: 30, height: 1 },
      makeSnapshot({ missionTitle: "A Very Long Mission Title That Should Be Truncated" }),
    );
    const text = buf.toString();
    expect(text.length).toBeLessThanOrEqual(30);
  });

  it("shows token placeholder when null", () => {
    const buf = new Buffer(80, 1);
    renderHeader(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    // No token counters should be rendered
    const text = buf.toString();
    expect(text).not.toContain("In:");
  });

  it("shows token counters when available", () => {
    const buf = new Buffer(120, 1);
    renderHeader(
      buf,
      { x: 0, y: 0, width: 120, height: 1 },
      makeSnapshot({ tokenCounters: { input: 1500, cached: 500, output: 300 } }),
    );
    const text = buf.toString();
    expect(text).toContain("In: 1.5k");
    expect(text).toContain("Out: 300");
  });
});
