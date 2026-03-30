import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderFooter } from "../../../../src/tui/panels/footer.js";
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

describe("renderFooter", () => {
  it("shows quit hint", () => {
    const buf = new Buffer(80, 1);
    renderFooter(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Quit");
  });

  it("shows navigate hint", () => {
    const buf = new Buffer(80, 1);
    renderFooter(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Navigate");
  });

  it("shows Pause when canPause is true", () => {
    const buf = new Buffer(80, 1);
    renderFooter(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot({ canPause: true }));
    const text = buf.toString();
    expect(text).toContain("Pause");
  });

  it("shows Resume when canResume is true", () => {
    const buf = new Buffer(80, 1);
    renderFooter(
      buf,
      { x: 0, y: 0, width: 80, height: 1 },
      makeSnapshot({ canPause: false, canResume: true }),
    );
    const text = buf.toString();
    expect(text).toContain("Resume");
  });

  it("shows Dir hint", () => {
    const buf = new Buffer(80, 1);
    renderFooter(buf, { x: 0, y: 0, width: 80, height: 1 }, makeSnapshot());
    const text = buf.toString();
    expect(text).toContain("Dir");
  });
});
