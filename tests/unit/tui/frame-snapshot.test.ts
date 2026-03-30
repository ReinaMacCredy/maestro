/**
 * Frame snapshot tests -- render full frames at known sizes
 * and verify content integrity.
 */
import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../src/tui/terminal/buffer.js";
import { createInitialState } from "../../../src/tui/state.js";
import type { MissionControlSnapshot } from "../../../src/tui/types.js";

// Re-export renderFrame for testing by importing the once-frame path
import { renderOnceFrame } from "../../../src/tui/index.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    missionId: "2026-03-30-001",
    missionTitle: "Full Pipeline Test",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 754_000,
    featureProgress: { done: 2, total: 4, active: 1 },
    tokenCounters: null,
    activeFeature: {
      id: "f2",
      title: "Database config",
      status: "in-progress",
      milestoneId: "m1",
      milestoneTitle: "Core Setup",
      workerType: "backend-worker",
      description: "Configure the database connection and migrations",
      preconditions: "Clean working directory",
      expectedBehavior: "Database connects and migrations run",
      verificationSteps: ["Run build", "Run lint", "Run tests"],
      dependsOn: ["f1"],
      fulfills: ["a-f2-1"],
      validTransitions: ["review"],
    },
    features: [
      { id: "f1", title: "Init project", status: "done", milestoneId: "m1", workerType: "backend-worker", hasReport: true },
      { id: "f2", title: "Database config", status: "in-progress", milestoneId: "m1", workerType: "backend-worker", hasReport: false },
      { id: "f3", title: "Auth endpoints", status: "pending", milestoneId: "m2", workerType: "backend-worker", hasReport: false },
      { id: "f4", title: "API docs", status: "pending", milestoneId: "m2", workerType: "backend-worker", hasReport: false },
    ],
    activeWorker: {
      featureId: "f2",
      featureTitle: "Database config",
      workerType: "backend-worker",
      status: "in-progress",
      elapsedMs: 252_000,
      report: null,
    },
    progressLog: [
      { timestamp: "2026-03-30T10:12:00.000Z", relativeMs: 720_000, kind: "feature", title: "f2 moved to in-progress" },
      { timestamp: "2026-03-30T10:10:00.000Z", relativeMs: 600_000, kind: "feature", title: "f1 moved to done" },
      { timestamp: "2026-03-30T10:05:00.000Z", relativeMs: 300_000, kind: "assertion", title: "a-f1-1: passed" },
      { timestamp: "2026-03-30T10:00:00.000Z", relativeMs: 0, kind: "mission", title: "Mission approved" },
    ],
    milestones: [
      { id: "m1", title: "Core Setup", status: "executing", order: 0 },
      { id: "m2", title: "API Layer", status: "pending", order: 1 },
    ],
    canPause: true,
    canResume: false,
    ...overrides,
  };
}

describe("frame rendering", () => {
  describe("standard size (120x32)", () => {
    it("contains mission control label", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Mission Control");
    });

    it("contains RUNNING status label", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("RUNNING");
    });

    it("contains feature titles", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Init project");
      expect(frame).toContain("Database config");
      expect(frame).toContain("Auth endpoints");
      expect(frame).toContain("API docs");
    });

    it("contains Features header", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Features");
    });

    it("contains progress counts", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("2/4");
    });

    it("contains worker info", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("backend-worker");
      expect(frame).toContain("Worker active");
    });

    it("contains progress log events", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Progress Log");
      expect(frame).toContain("f2 moved to in-progress");
    });

    it("contains footer hints", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Features");
      expect(frame).toContain("Back To Orchestrator");
    });
  });

  describe("empty mission", () => {
    it("shows meaningful placeholder when no features", () => {
      const frame = renderOnceFrame({
        snapshot: makeSnapshot({
          features: [],
          activeFeature: null,
          activeWorker: null,
          featureProgress: { done: 0, total: 0, active: 0 },
          progressLog: [
            { timestamp: "2026-03-30T10:00:00.000Z", relativeMs: 0, kind: "mission", title: "Mission created" },
          ],
        }),
      });
      expect(frame).toContain("Mission Control");
      expect(frame).toContain("No active feature");
      expect(frame).toContain("No active workers");
    });
  });

  describe("completed mission", () => {
    it("shows completed state", () => {
      const frame = renderOnceFrame({
        snapshot: makeSnapshot({
          effectiveStatus: "completed",
          missionStatus: "completed",
          featureProgress: { done: 4, total: 4, active: 0 },
          activeWorker: null,
          canPause: false,
          canResume: false,
          features: [
            { id: "f1", title: "Init project", status: "done", milestoneId: "m1", workerType: "backend-worker", hasReport: true },
            { id: "f2", title: "Database config", status: "done", milestoneId: "m1", workerType: "backend-worker", hasReport: true },
            { id: "f3", title: "Auth endpoints", status: "done", milestoneId: "m2", workerType: "backend-worker", hasReport: true },
            { id: "f4", title: "API docs", status: "done", milestoneId: "m2", workerType: "backend-worker", hasReport: true },
          ],
        }),
      });
      expect(frame).toContain("COMPLETED");
      expect(frame).toContain("4/4");
    });
  });

  describe("paused mission", () => {
    it("shows resume hint", () => {
      const frame = renderOnceFrame({
        snapshot: makeSnapshot({
          effectiveStatus: "paused",
          missionStatus: "paused",
          canPause: false,
          canResume: true,
        }),
      });
      expect(frame).toContain("PAUSED");
      expect(frame).toContain("Resume");
    });
  });

  describe("feature detail fields", () => {
    it("shows preconditions when available", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("Preconditions");
      expect(frame).toContain("Clean working directory");
    });

    it("shows verification steps when panel has enough height", () => {
      // Use a snapshot without preconditions/expectedBehavior to leave room
      const snap = makeSnapshot();
      snap.activeFeature = {
        ...snap.activeFeature!,
        preconditions: undefined,
        expectedBehavior: undefined,
      };
      const frame = renderOnceFrame({ snapshot: snap });
      expect(frame).toContain("Verification Steps");
      expect(frame).toContain("Run build");
    });

    it("shows worker type", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("skill backend-worker");
    });
  });
});
