import { describe, expect, it } from "bun:test";

import { buildModalOptions } from "../../../src/tui/app/modal-builders.js";
import { createInitialState, reduce } from "../../../src/tui/state/reducer.js";
import type { MissionControlSnapshot } from "../../../src/tui/state/types.js";

function makeSnapshot(): MissionControlSnapshot {
  return {
    mode: "mission",
    missionId: "mission-1",
    missionTitle: "Mission 1",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 1, active: 1 },
    statusProgress: {
      completed: 0,
      total: 1,
      inFlight: 1,
      blocked: 0,
      queued: 0,
      completionPct: 0,
    },
    tokenCounters: null,
    features: [
      {
        id: "f1",
        title: "Feature 1",
        status: "assigned",
        milestoneId: "m1",
        workerType: "backend",
        hasReport: false,
      },
    ],
    milestones: [],
    activeFeature: null,
    activeWorker: null,
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    configInspector: null,
    runtimeProcesses: [],
    progressLog: [],
    canPause: false,
    canResume: false,
    memory: {
      stats: {
        corrections: { total: 1, hard: 1, soft: 0 },
        learnings: { rawCount: 2, compiledAt: "2026-04-06T01:00:00.000Z", staleDays: 0 },
        ratchet: { assertions: 1, lastResult: "pass" },
        graph: { projects: 2, links: 1 },
      },
      corrections: [{
        id: "corr-1",
        rule: "Use bun, not npm",
        source: "Plan verification",
        trigger: { keywords: ["package", "install"], fileGlobs: ["package.json"] },
        severity: "hard",
        createdAt: "2026-04-06T00:00:00.000Z",
        updatedAt: "2026-04-06T00:00:00.000Z",
      }],
      rawLearnings: [
        {
          sessionDate: "2026-04-06T00:00:00.000Z",
          content: "Always wire snapshot data before the TUI.",
          branch: "feat/missionControl",
        },
        {
          sessionDate: "2026-04-05T00:00:00.000Z",
          content: "Render-check is useful after TUI changes.",
          branch: "feat/missionControl",
        },
      ],
      compiledLearnings: {
        compiledAt: "2026-04-06T01:00:00.000Z",
        summary: "Snapshot-backed data keeps previews honest.",
        rawCount: 2,
      },
      ratchetSuite: {
        assertions: [{
          id: "ratchet-1",
          correctionId: "corr-1",
          rule: "Use bun, not npm",
          check: "rg -n \"npm\"",
          createdAt: "2026-04-06T02:00:00.000Z",
        }],
      },
      ratchetBaseline: {
        passCount: 1,
        lastRunAt: "2026-04-06T03:00:00.000Z",
      },
      graphContext: {
        currentProject: { name: "maestro", path: "/tmp/maestro", role: "cli" },
        relationships: [{
          direction: "outgoing",
          project: { name: "maestro-web", path: "/tmp/maestro-web", role: "frontend" },
          edge: { from: "maestro", to: "maestro-web", relation: "exposes", detail: "mcp-tools" },
        }],
        totalProjects: 2,
        totalEdges: 1,
      },
    },
    memoryStats: {
      corrections: { total: 1, hard: 1, soft: 0 },
      learnings: { rawCount: 2, compiledAt: "2026-04-06T01:00:00.000Z", staleDays: 0 },
      ratchet: { assertions: 1, lastResult: "pass" },
      graph: { projects: 2, links: 1 },
    },
    home: null,
  };
}

describe("memory modal", () => {
  it("renders an overview panel with memory stats", () => {
    const state = createInitialState(makeSnapshot());
    const memoryState = reduce(state, { type: "open-memory" });
    const modal = buildModalOptions(memoryState);

    expect(modal?.mode).toBe("info");
    if (!modal || modal.mode !== "info") return;
    expect(modal.eyebrow).toContain("[overview]");
    expect(modal.items.some((item) => item.text.includes("1 total"))).toBe(true);
  });

  it("cycles tabs and resets the selected item index", () => {
    const state = reduce(createInitialState(makeSnapshot()), { type: "open-memory" });
    const correctionsState = reduce(state, { type: "memory-next-tab" });
    const movedState = reduce(
      { ...correctionsState, modal: correctionsState.modal.kind === "memory" ? { ...correctionsState.modal, selectedItemIndex: 3 } : correctionsState.modal },
      { type: "memory-next-tab" },
    );

    expect(correctionsState.modal.kind).toBe("memory");
    if (correctionsState.modal.kind === "memory") {
      expect(correctionsState.modal.tab).toBe("corrections");
    }
    if (movedState.modal.kind === "memory") {
      expect(movedState.modal.tab).toBe("learnings");
      expect(movedState.modal.selectedItemIndex).toBe(0);
    }
  });

  it("renders correction detail and graph relationships from snapshot data", () => {
    const state = createInitialState(makeSnapshot());
    const correctionState = reduce(reduce(state, { type: "open-memory" }), { type: "memory-next-tab" });
    const memoryModal = buildModalOptions(correctionState);

    expect(memoryModal?.mode).toBe("split");
    if (!memoryModal || memoryModal.mode !== "split") return;
    expect(memoryModal.items[0]?.label).toContain("Use bun, not npm");
    expect(memoryModal.detailItems.some((item) => item.text.includes("Plan verification"))).toBe(true);

    const graphState = reduce(state, { type: "open-graph" });
    const graphModal = buildModalOptions(graphState);

    expect(graphModal?.mode).toBe("split");
    if (!graphModal || graphModal.mode !== "split") return;
    expect(graphModal.items[0]?.label).toContain("maestro-web");
    expect(graphModal.detailItems.some((item) => item.text.includes("mcp-tools"))).toBe(true);
  });
});
