/**
 * Frame snapshot tests -- render full frames at known sizes
 * and verify content integrity.
 */
import { describe, expect, it } from "bun:test";
import { Buffer } from "../../../src/tui/terminal/buffer.js";
import { createInitialState } from "../../../src/tui/state.js";
import { PALETTE } from "../../../src/tui/theme.js";
import type { MissionControlSnapshot } from "../../../src/tui/types.js";

// Re-export renderFrame for testing by importing the once-frame path
import { renderFrame, renderOnceFrame } from "../../../src/tui/index.js";

function withTerminalSize<T>(width: number, height: number, run: () => T): T {
  const columnsDescriptor = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  const rowsDescriptor = Object.getOwnPropertyDescriptor(process.stdout, "rows");

  Object.defineProperty(process.stdout, "columns", {
    configurable: true,
    value: width,
  });
  Object.defineProperty(process.stdout, "rows", {
    configurable: true,
    value: height,
  });

  try {
    return run();
  } finally {
    if (columnsDescriptor) {
      Object.defineProperty(process.stdout, "columns", columnsDescriptor);
    } else {
      delete (process.stdout as { columns?: number }).columns;
    }

    if (rowsDescriptor) {
      Object.defineProperty(process.stdout, "rows", rowsDescriptor);
    } else {
      delete (process.stdout as { rows?: number }).rows;
    }
  }
}

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  const base: MissionControlSnapshot = {
      mode: "mission",
      missionId: "2026-03-30-001",
      missionTitle: "Full Pipeline Test",
      missionStatus: "executing",
      effectiveStatus: "executing",
      elapsedMs: 754_000,
    featureProgress: { done: 2, total: 4, active: 1 },
    statusProgress: {
      completed: 2,
      total: 4,
      inFlight: 1,
      blocked: 0,
      queued: 2,
      completionPct: 50,
      },
      tokenCounters: null,
      missionOverview: null,
      session: {
        agent: "codex",
        sessionId: "5634c102-9871-4001-86f8-89399077624e",
        branch: "main",
        workingTreeClean: false,
        diffStat: "+42 -11",
        changedFiles: ["src/db.ts", "src/config.ts", "tests/db.test.ts"],
        fileChanges: [
          { path: "src/db.ts", kind: "added" },
          { path: "src/config.ts", kind: "added" },
          { path: "tests/db.test.ts", kind: "deleted" },
        ],
      },
      pendingHandoffs: [],
      configSummary: {
        configSource: "project",
        cassAvailable: true,
        gitAvailable: true,
        checks: [
          { name: "git", status: "ok", message: "Git repository detected" },
          { name: "project-config", status: "ok", message: "Project config present" },
        ],
        missionDirectory: ".maestro/missions/2026-03-30-001",
        workerTypes: ["backend-worker"],
      },
      runtimeProcesses: [
        {
          featureId: "f2",
          title: "Database config",
          status: "in-progress",
          workerType: "backend-worker",
          hasReport: false,
          isLive: true,
        },
      ],
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
        blockedBy: [],
        unblocks: [{ id: "f3", title: "Auth endpoints", status: "pending" }],
        fulfills: ["a-f2-1"],
        validTransitions: ["review"],
      },
      features: [
        { id: "f1", title: "Init project", status: "done", milestoneId: "m1", workerType: "backend-worker", hasReport: true, blockedByIds: [] },
        { id: "f2", title: "Database config", status: "in-progress", milestoneId: "m1", workerType: "backend-worker", hasReport: false, blockedByIds: [] },
        { id: "f3", title: "Auth endpoints", status: "pending", milestoneId: "m2", workerType: "backend-worker", hasReport: false, blockedByIds: ["f2"], blockedByLabel: "f2" },
        { id: "f4", title: "API docs", status: "pending", milestoneId: "m2", workerType: "backend-worker", hasReport: false, blockedByIds: [] },
      ],
      taskPreviews: [],
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
      home: null,
    };

  const snapshot = { ...base, ...overrides };
  snapshot.taskPreviews = snapshot.taskPreviews ?? (snapshot.activeFeature ? [snapshot.activeFeature] : []);
  snapshot.missionOverview = snapshot.missionOverview ?? {
    missionLabel: `Mission: ${snapshot.missionTitle}`,
    statusLabel: snapshot.effectiveStatus,
    activeCount: snapshot.featureProgress.active,
    doneCount: snapshot.featureProgress.done,
    totalCount: snapshot.featureProgress.total,
    blockedCount: snapshot.statusProgress.blocked,
    currentMilestone: snapshot.milestones[0]?.title ?? null,
    gateLabel: snapshot.gateLabel ?? null,
    agentSummary: [{ agent: "codex", count: 1 }],
      dependencyMap: [
        {
          root: { id: "f2", title: "Database config", status: "in-progress" },
          primaryDependent: { id: "f3", title: "Auth endpoints", status: "pending" },
          primaryDependentBlockedByCount: 1,
          hiddenDependentCount: 0,
        },
      ],
  };

  return snapshot;
}

function renderPreviewFrame(snapshot: MissionControlSnapshot): string {
  return withTerminalSize(120, 40, () => {
    const buf = new Buffer(120, 40);
    const state = createInitialState(snapshot);
    state.leftPaneMode = "preview";
    renderFrame(buf, state);
    return buf.toString();
  });
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
    });

      it("contains Tasks header", () => {
        const frame = renderOnceFrame({ snapshot: makeSnapshot() });
        expect(frame).toContain("Tasks");
      });

    it("contains progress counts", () => {
      const frame = renderOnceFrame({ snapshot: makeSnapshot() });
      expect(frame).toContain("2/4");
    });

      it("contains mission overview info", () => {
        const frame = renderOnceFrame({ snapshot: makeSnapshot() });
        expect(frame).toContain("Mission Overview");
        expect(frame).toContain("Mission: Full Pipeline Test");
        expect(frame).toContain("status     running");
        expect(frame).toContain("agents");
        expect(frame).toContain("Dependency Map");
      });

      it("contains timeline and session labels", () => {
        const frame = renderOnceFrame({ snapshot: makeSnapshot() });
        expect(frame).toContain("Timeline");
        expect(frame).toContain("Session / Changes");
      });

      it("contains footer hints", () => {
        const frame = renderOnceFrame({ snapshot: makeSnapshot() });
          expect(frame).toContain("Tasks");
      expect(frame).toContain("Handoffs");
      expect(frame).toContain("Config");
      expect(frame).toContain("Runtime");
          expect(frame).toContain("Commands");
          expect(frame).toContain("Exit");
        });

      it("contains blocked gate context when the active milestone is a gate", () => {
        const frame = renderOnceFrame({
          snapshot: makeSnapshot({
            milestones: [
              { id: "m1", title: "Plan Review", status: "executing", order: 0, kind: "gate", profile: "plan-review" },
              { id: "m2", title: "Implementation", status: "pending", order: 1, kind: "work", profile: "implementation" },
            ],
            gateBlocked: true,
            gateLabel: "Plan Review",
          }),
        });

        expect(frame).toContain("PLAN-REVIEW");
        expect(frame).toContain("BLOCKED");
        expect(frame).toContain("Milestone:");
        expect(frame).toContain("Gate:");
      });
    });

  describe("empty mission", () => {
    it("shows meaningful placeholder when no features", () => {
        const frame = renderOnceFrame({
          snapshot: makeSnapshot({
            features: [],
            activeFeature: null,
            activeWorker: null,
            session: {
              branch: "main",
              workingTreeClean: true,
              diffStat: "+0 -0",
              changedFiles: [],
            },
            featureProgress: { done: 0, total: 0, active: 0 },
            statusProgress: {
            completed: 0,
            total: 0,
            inFlight: 0,
            blocked: 0,
            queued: 0,
            completionPct: 0,
          },
          progressLog: [
            { timestamp: "2026-03-30T10:00:00.000Z", relativeMs: 0, kind: "mission", title: "Mission created" },
          ],
          }),
          });
          expect(frame).toContain("Mission Control");
            expect(frame).toContain("Mission Overview");
            expect(frame).toContain("0/0");
          });
        });

  describe("completed mission", () => {
    it("shows completed state", () => {
      const frame = renderOnceFrame({
        snapshot: makeSnapshot({
          effectiveStatus: "completed",
          missionStatus: "completed",
          featureProgress: { done: 4, total: 4, active: 0 },
          statusProgress: {
            completed: 4,
            total: 4,
            inFlight: 0,
            blocked: 0,
            queued: 0,
            completionPct: 100,
          },
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
          expect(frame).toContain("Runtime");
        });
    });

    describe("feature detail fields", () => {
        it("shows preconditions when available", () => {
          const frame = renderPreviewFrame(makeSnapshot());
          expect(frame).toContain("Focus / Preview");
          expect(frame).toContain("worker");
          expect(frame).toContain("backend-worker");
        });

      it("shows verification steps when panel has enough height", () => {
        const frame = renderPreviewFrame(makeSnapshot());
        expect(frame).toContain("blocked by");
        expect(frame).toContain("unblocks");
      });

      it("shows worker type", () => {
        const frame = renderPreviewFrame(makeSnapshot());
        expect(frame).toContain("agent");
        expect(frame).toContain("codex");
      });
  });

  describe("chrome layout", () => {
    it("renders a full outer frame with connected dividers", () => {
      const frame = withTerminalSize(80, 24, () => renderOnceFrame({ snapshot: makeSnapshot() }));
      const lines = frame.split("\n");

      expect(lines[0]?.startsWith("┌")).toBe(true);
      expect(lines[0]?.endsWith("┐")).toBe(true);
      expect(lines.at(-1)?.startsWith("└")).toBe(true);
      expect(lines.at(-1)?.endsWith("┘")).toBe(true);
      expect(frame).toContain("├");
      expect(frame).toContain("┤");
      expect(frame).toContain("┬");
      expect(frame).toContain("┴");
      expect(frame).toContain("┼");
    });

    it("keeps the full chrome in empty-state frames", () => {
      const frame = withTerminalSize(80, 24, () =>
        renderOnceFrame({
            snapshot: makeSnapshot({
              features: [],
              activeFeature: null,
              activeWorker: null,
              session: {
                branch: "main",
                workingTreeClean: true,
                diffStat: "+0 -0",
                changedFiles: [],
              },
              featureProgress: { done: 0, total: 0, active: 0 },
              statusProgress: {
              completed: 0,
              total: 0,
              inFlight: 0,
              blocked: 0,
              queued: 0,
              completionPct: 0,
            },
            progressLog: [],
          }),
        }));

          expect(frame).toContain("┌");
          expect(frame).toContain("└");
          expect(frame).toContain("│");
            expect(frame).toContain("No active work");
            expect(frame).toContain("Timeline");
          });

      it("renders bordered chrome for narrow-but-valid terminals", () => {
        const frame = withTerminalSize(60, 18, () => renderOnceFrame({ snapshot: makeSnapshot() }));
        const lines = frame.split("\n");

      expect(lines[0]?.startsWith("┌")).toBe(true);
      expect(lines[0]?.endsWith("┐")).toBe(true);
        expect(frame).not.toContain("undefined");
        expect(lines.every((line) => line.length <= 60)).toBe(true);
      });

      it("renders a guided home layout without mission context", () => {
          const frame = withTerminalSize(80, 24, () => renderOnceFrame({
            snapshot: makeSnapshot({
              mode: "home",
              missionTitle: "No project detected",
              features: [],
              activeFeature: null,
              activeWorker: null,
              session: null,
              progressLog: [],
              statusProgress: {
                completed: 0,
              total: 0,
              inFlight: 0,
              blocked: 0,
              queued: 0,
              completionPct: 0,
            },
              home: {
                headline: "No project detected",
                summary: "Open a git repository to track missions here.",
              locationLabel: "Outside a git repository",
              checks: [
                { name: "git", status: "fail", message: "Not inside a git repository", fix: "Run: git init" },
                { name: "global-config", status: "warn", message: "No global config found", fix: "Run: maestro init --global" },
              ],
              actions: [
                { label: "Create a project repo", command: "git init", detail: "Initialize this folder first." },
                { label: "Run environment checks", command: "maestro doctor", detail: "Verify your environment." },
              ],
              pendingHandoffs: [],
            },
          }),
        }));

          expect(frame).toContain("HOME");
          expect(frame).toContain("Overview");
          expect(frame).toContain("Environment");
          expect(frame).toContain("Pending Handoffs");
          expect(frame).toContain("Ctrl+P Commands");
        });

        it("renders the configuration modal with command-palette styling", () => {
          const frame = withTerminalSize(90, 28, () => {
            const buf = new Buffer(90, 28);
              const state = createInitialState(makeSnapshot());
              state.modal = { kind: "config" };
            renderFrame(buf, state);
          return buf.toString();
        });

        expect(frame).toContain("Config");
        expect(frame).toContain("Config source: project");
        expect(frame).toContain("Mission Directory");
        expect(frame).toContain(".maestro/missions/2026-03-30-001");
        expect(frame).toContain("Esc close");
          expect(frame).toContain("Workers");
          expect(frame).toContain("backend-work");
        });

        it("shows left-back plus escape-close copy for palette-launched detail overlays", () => {
          const frame = withTerminalSize(90, 28, () => {
            const buf = new Buffer(90, 28);
            const state = createInitialState(makeSnapshot());
            state.modal = { kind: "config", returnTarget: "command-palette" };
            renderFrame(buf, state);
            return buf.toString();
          });

          expect(frame).toContain("Left back · Esc close");
        });

        it("renders the command palette and dims the dashboard behind it", () => {
        const buf = new Buffer(90, 28);
        const state = createInitialState(makeSnapshot());
        state.modal = { kind: "command-palette", query: "pro", selectedCommandIndex: 0 };

        renderFrame(buf, state);

        const frame = buf.toString();
        expect(frame).toContain("Commands");
        expect(frame).toContain("Navigate");
        expect(frame).toContain("Runtime");
        expect(frame).toContain("Enter open · Esc close");
        expect(buf.getCell(1, 1)?.bg).toBe(PALETTE.overlayBackdropBg);
        expect(buf.getCell(1, 1)?.dim).toBe(true);
      });
      });
    });
