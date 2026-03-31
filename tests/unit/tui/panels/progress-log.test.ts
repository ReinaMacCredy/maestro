import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderProgressLog } from "../../../../src/tui/panels/progress-log.js";
import { PALETTE } from "../../../../src/tui/theme.js";
import type { MissionControlEvent } from "../../../../src/tui/types.js";

const EVENTS: readonly MissionControlEvent[] = [
  { timestamp: "2026-03-30T10:12:00.000Z", relativeMs: 720_000, kind: "feature", title: "f1 moved to done" },
  { timestamp: "2026-03-30T10:10:00.000Z", relativeMs: 600_000, kind: "mission", title: "Mission approved" },
  { timestamp: "2026-03-30T10:08:00.000Z", relativeMs: 480_000, kind: "worker", title: "Worker report received" },
];

describe("renderProgressLog", () => {
  it("uses brighter age text", () => {
    const buf = new Buffer(48, 8);
    renderProgressLog(buf, { x: 0, y: 0, width: 48, height: 8 }, EVENTS);

    const ageCell = buf.getCell(2, 3);
    expect(ageCell?.fg).toBe(PALETTE.gray);
  });

  it("uses bright text for non-feature events", () => {
    const buf = new Buffer(48, 8);
    renderProgressLog(buf, { x: 0, y: 0, width: 48, height: 8 }, EVENTS);

    const missionTitleCell = buf.getCell(3, 10);
    expect(missionTitleCell?.char).toBe("M");
    expect(missionTitleCell?.fg).toBe(PALETTE.brightWhite);
  });

  it("renders from the requested scroll offset", () => {
    const buf = new Buffer(48, 8);
    renderProgressLog(buf, { x: 0, y: 0, width: 48, height: 8 }, EVENTS, undefined, 1);

    const text = buf.toString();
    expect(text).toContain("Mission approved");
    expect(text).toContain("Worker report received");
    expect(text).not.toContain("f1 moved to done");
  });
});
