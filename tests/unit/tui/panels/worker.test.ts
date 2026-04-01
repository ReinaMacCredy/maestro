import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../../src/tui/terminal/buffer.js";
import { renderSessionSidebar, renderWorkerPanel } from "../../../../src/tui/panels/worker.js";
import { PALETTE } from "../../../../src/tui/theme.js";
import type { MissionControlSnapshot } from "../../../../src/tui/state/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "2026-03-30-001",
    missionTitle: "Test Mission",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 120_000,
    featureProgress: { done: 1, total: 3, active: 1 },
    statusProgress: {
      completed: 1,
      total: 3,
      inFlight: 1,
      blocked: 0,
      queued: 1,
      completionPct: 33,
    },
    tokenCounters: null,
    session: {
      branch: "main",
      workingTreeClean: false,
      diffStat: "+4 -1",
      changedFiles: ["src/tui/worker.ts", "tests/unit/tui/panels/worker.test.ts", "src/tui/index.ts"],
    },
    pendingHandoffs: [],
    configSummary: {
      configSource: "project",
      cassAvailable: true,
      gitAvailable: true,
      checks: [],
      missionDirectory: ".maestro/missions/2026-03-30-001",
      workerTypes: ["backend-worker"],
    },
    runtimeProcesses: [],
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

describe("renderWorkerPanel", () => {
  it("renders the new activity/session two-pane structure", () => {
    const buf = new Buffer(80, 8);
    renderWorkerPanel(buf, { x: 0, y: 0, width: 80, height: 8 }, makeSnapshot());

    const text = buf.toString();
    expect(text).toContain("Activity");
    expect(text).toContain("Session");
    expect(text).toContain("Task");
    expect(text).toContain("Branch");
    expect(text).toContain("Changes");
  });

  it("shows structured task rows for the next feature", () => {
    const buf = new Buffer(80, 6);
    renderWorkerPanel(buf, { x: 0, y: 0, width: 80, height: 6 }, makeSnapshot({
      activeFeature: {
        id: "f2",
        title: "Database config",
        status: "pending",
        milestoneId: "m1",
        milestoneTitle: "Core Setup",
        workerType: "backend-worker",
        description: "Configure the database",
        preconditions: undefined,
        expectedBehavior: undefined,
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        validTransitions: ["assigned"],
      },
    }));

    const text = buf.toString();
    expect(text).toContain("Task");
    expect(text).toContain("Database config");
    expect(text).toContain("State");
    expect(text).toContain("Waiting to start next feature");
  });

  it("shows stale runtime messaging for the active worker", () => {
    const buf = new Buffer(90, 6);
    renderWorkerPanel(buf, { x: 0, y: 0, width: 90, height: 6 }, makeSnapshot({
      activeWorker: {
        featureId: "f2",
        featureTitle: "Database config",
        workerType: "backend-worker",
        status: "in-progress",
        elapsedMs: 30_000,
        report: null,
        runtimeState: "stale",
        lastSeenAgeMs: 120_000,
      },
    }));

    const text = buf.toString();
    expect(text).toContain("Worker heartbeat stale");
    expect(text).toContain("Recovery review or manual retry");
  });

    it("shows recoverable guidance for the next feature", () => {
      const buf = new Buffer(90, 6);
      renderWorkerPanel(buf, { x: 0, y: 0, width: 90, height: 6 }, makeSnapshot({
      activeWorker: null,
      activeFeature: {
        id: "f2",
        title: "Database config",
        status: "pending",
        milestoneId: "m1",
        milestoneTitle: "Core Setup",
        workerType: "backend-worker",
        description: "Configure the database",
        preconditions: undefined,
        expectedBehavior: undefined,
        verificationSteps: [],
        dependsOn: [],
        fulfills: [],
        validTransitions: ["assigned"],
        runtimeState: "recoverable",
        retryCount: 1,
      },
    }));

      const text = buf.toString();
      expect(text).toContain("Recovery ready");
      expect(text).toContain("Prompt generation can resume");
    });

  it("sanitizes failure reasons before writing them into the terminal buffer", () => {
      const buf = new Buffer(90, 6);
      renderWorkerPanel(buf, { x: 0, y: 0, width: 90, height: 6 }, makeSnapshot({
        activeWorker: {
          featureId: "f2",
          featureTitle: "Database config",
          workerType: "backend-worker",
          status: "in-progress",
          elapsedMs: 30_000,
          report: null,
          runtimeState: "failed",
          failureReason: "\u001b[2Jworker vanished\u0007",
        },
      }));

      const text = buf.toString();
      expect(text).toContain("worker vanished");
      expect(text).not.toContain("\u001b");
      expect(text).not.toContain("\u0007");
      expect(text).not.toContain("[2J");
    });

  it("uses change-kind colors in the Session / Changes file list", () => {
      const buf = new Buffer(80, 10);
      renderSessionSidebar(buf, { x: 0, y: 0, width: 80, height: 10 }, makeSnapshot({
        session: {
          agent: "codex",
          sessionId: "5634c102-9871-4001-86f8-89399077624e",
          branch: "main",
          workingTreeClean: false,
          diffStat: "+4 -1",
          changedFiles: ["src/tui/worker.ts", "src/tui/index.ts", "tests/unit/tui/panels/worker.test.ts"],
          fileChanges: [
            { path: "src/tui/worker.ts", kind: "modified" },
            { path: "src/tui/index.ts", kind: "added" },
            { path: "tests/unit/tui/panels/worker.test.ts", kind: "deleted" },
          ],
        },
      }));

      expect(buf.getCell(6, 11)?.char).toBe("~");
      expect(buf.getCell(6, 11)?.fg).toBe(PALETTE.yellow);
      expect(buf.getCell(6, 13)?.fg).toBe(PALETTE.yellow);

      expect(buf.getCell(7, 1)?.char).toBe("+");
      expect(buf.getCell(7, 1)?.fg).toBe(PALETTE.green);
      expect(buf.getCell(7, 3)?.fg).toBe(PALETTE.green);

      expect(buf.getCell(8, 1)?.char).toBe("-");
      expect(buf.getCell(8, 1)?.fg).toBe(PALETTE.red);
      expect(buf.getCell(8, 3)?.fg).toBe(PALETTE.red);
    });

    it("colors the Changes summary diff counts", () => {
      const buf = new Buffer(80, 8);
      renderSessionSidebar(buf, { x: 0, y: 0, width: 80, height: 8 }, makeSnapshot({
        session: {
          agent: "codex",
          sessionId: "5634c102-9871-4001-86f8-89399077624e",
          branch: "main",
          workingTreeClean: false,
          diffStat: "+118 -17",
          changedFiles: ["package.json", "src/tui/index.ts"],
          fileChanges: [
            { path: "package.json", kind: "modified" },
            { path: "src/tui/index.ts", kind: "added" },
          ],
        },
      }));

      expect(buf.toString()).toContain("Changes");
      expect(buf.getCell(5, 21)?.char).toBe("+");
      expect(buf.getCell(5, 21)?.fg).toBe(PALETTE.green);
      expect(buf.getCell(5, 26)?.char).toBe("-");
      expect(buf.getCell(5, 26)?.fg).toBe(PALETTE.red);
    });
  });
