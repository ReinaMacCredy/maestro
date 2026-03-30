import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderFeatureDot, renderMissionDot } from "../../../../src/tui/widgets/status-dot.js";
import { FEATURE_STATUS_COLOR, MISSION_STATUS_COLOR } from "../../../../src/tui/theme.js";
import type { FeatureStatus, MissionStatus } from "../../../../src/domain/mission-types.js";

describe("renderFeatureDot", () => {
  const statuses: FeatureStatus[] = ["pending", "assigned", "in-progress", "review", "done", "blocked"];

  for (const status of statuses) {
    it(`renders dot for ${status} with correct color`, () => {
      const buf = new Buffer(3, 1);
      renderFeatureDot(buf, 0, 0, status);
      const cell = buf.getCell(0, 0);
      expect(cell).toBeDefined();
      expect(cell!.fg).toBe(FEATURE_STATUS_COLOR[status]);
      expect(cell!.char.length).toBe(1);
    });
  }

  it("uses 'x' dot for blocked status", () => {
    const buf = new Buffer(3, 1);
    renderFeatureDot(buf, 0, 0, "blocked");
    expect(buf.getCell(0, 0)!.char).toBe("x");
  });

  it("uses 'o' dot for pending status", () => {
    const buf = new Buffer(3, 1);
    renderFeatureDot(buf, 0, 0, "pending");
    expect(buf.getCell(0, 0)!.char).toBe("o");
  });

  it("uses '*' dot for done status", () => {
    const buf = new Buffer(3, 1);
    renderFeatureDot(buf, 0, 0, "done");
    expect(buf.getCell(0, 0)!.char).toBe("*");
  });
});

describe("renderMissionDot", () => {
  const statuses: MissionStatus[] = ["draft", "approved", "executing", "paused", "completed", "failed"];

  for (const status of statuses) {
    it(`renders dot for ${status} with correct color`, () => {
      const buf = new Buffer(3, 1);
      renderMissionDot(buf, 0, 0, status);
      const cell = buf.getCell(0, 0);
      expect(cell).toBeDefined();
      expect(cell!.fg).toBe(MISSION_STATUS_COLOR[status]);
      expect(cell!.char).toBe("*");
    });
  }
});
