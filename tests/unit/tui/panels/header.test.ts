import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderHeader } from "../../../../src/tui/panels/header.js";
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
    tokenCounters: null,
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

describe("renderHeader", () => {
  it("renders Mission Control label", () => {
    const buf = new Buffer(80, 1);
    renderHeader(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Mission Control");
  });

  it("shows TIME with elapsed duration", () => {
    const buf = new Buffer(80, 1);
    renderHeader(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("TIME 2m");
  });

  it("shows no token labels when counters are null", () => {
    const buf = new Buffer(80, 1);
    renderHeader(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).not.toContain("Input");
  });

  it("shows token counters when available", () => {
    const buf = new Buffer(120, 1);
    renderHeader(
      buf,
      { x: 0, y: 0, width: 120, height: 1 },
      makeSnapshot({ tokenCounters: { input: 1500, cached: 500, output: 300 } }),
    );
    const text = buf.toString();
    expect(text).toContain("Input 1.5k");
    expect(text).toContain("Output 300");
  });
});
