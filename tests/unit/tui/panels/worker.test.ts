import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderWorkerPanel } from "../../../../src/tui/panels/worker.js";
import { PALETTE } from "../../../../src/tui/theme.js";
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

describe("renderWorkerPanel", () => {
  it("renders an explicit workers heading for the empty state", () => {
    const buf = new Buffer(80, 4);
    renderWorkerPanel(buf, { x: 0, y: 0, width: 80, height: 4 }, makeSnapshot());

    const headingCell = buf.getCell(0, 1);
    expect(headingCell?.char).toBe("W");
    expect(headingCell?.fg).toBe(PALETTE.brightWhite);

    const emptyStateCell = buf.getCell(1, 1);
    expect(emptyStateCell?.char).toBe("N");
    expect(emptyStateCell?.fg).toBe(PALETTE.gray);
  });
});
