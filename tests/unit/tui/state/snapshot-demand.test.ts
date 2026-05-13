import { describe, expect, it } from "bun:test";
import { buildMissionControlSnapshotDemand } from "@/tui/state/snapshot-demand.js";

describe("buildMissionControlSnapshotDemand", () => {
  it("loads task-board data for whole-snapshot read modes", () => {
    expect(buildMissionControlSnapshotDemand({ mode: "json" })).toEqual({ includeTaskBoard: true });
    expect(buildMissionControlSnapshotDemand({ mode: "render-check" })).toEqual({ includeTaskBoard: true });
    expect(buildMissionControlSnapshotDemand({ mode: "preview-all" })).toEqual({ includeTaskBoard: true });
  });

  it("loads task-board data for every preview screen (home projection branches on task count)", () => {
    expect(buildMissionControlSnapshotDemand({ mode: "preview-screen", screen: "tasks" })).toEqual({
      includeTaskBoard: true,
    });
    expect(buildMissionControlSnapshotDemand({ mode: "preview-screen", screen: "dashboard" })).toEqual({
      includeTaskBoard: true,
    });
  });

  it("does not force optional read-model data for interactive startup", () => {
    expect(buildMissionControlSnapshotDemand({ mode: "interactive" })).toEqual({});
  });
});
