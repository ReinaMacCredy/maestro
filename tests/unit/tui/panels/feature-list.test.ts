import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderFeatureList } from "../../../../src/tui/panels/feature-list.js";
import { PALETTE } from "../../../../src/tui/theme.js";
import type { MissionControlSnapshot } from "../../../../src/tui/types.js";

function makeSnapshot(): MissionControlSnapshot {
  return {
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 754_000,
    featureProgress: { done: 1, total: 3, active: 1 },
    tokenCounters: null,
    activeFeature: null,
    features: [
      { id: "f1", title: "Setup project structure", status: "done", milestoneId: "m1", workerType: "worker", hasReport: true },
      { id: "f2", title: "Configure database", status: "pending", milestoneId: "m1", workerType: "worker", hasReport: false },
      { id: "f3", title: "Implement API endpoints", status: "pending", milestoneId: "m1", workerType: "worker", hasReport: false },
    ],
    activeWorker: null,
    progressLog: [],
    milestones: [],
    canPause: true,
    canResume: false,
  };
}

describe("renderFeatureList", () => {
  it("uses bright text for non-selected feature titles", () => {
    const buf = new Buffer(48, 8);
    renderFeatureList(buf, { x: 0, y: 0, width: 48, height: 8 }, makeSnapshot(), 2);

    const firstTitleCell = buf.getCell(2, 4);
    expect(firstTitleCell?.char).toBe("S");
    expect(firstTitleCell?.fg).toBe(PALETTE.brightWhite);
  });

  it("uses bright text for the feature count", () => {
    const buf = new Buffer(48, 8);
    renderFeatureList(buf, { x: 0, y: 0, width: 48, height: 8 }, makeSnapshot(), 1);

    const countCell = buf.getCell(0, 43);
    expect(countCell?.char).toBe("1");
    expect(countCell?.fg).toBe(PALETTE.brightWhite);
  });
});
