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
    session: null,
    pendingHandoffs: [],
    configSummary: null,
    configInspector: {
      tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
      rowsByTab: {
        overview: [],
        effective: [],
        project: [],
        global: [],
        defaults: [],
        workers: [],
        plan: [],
        doctor: [],
        memory: [
          {
            keyPath: "memory.enabled",
            label: "Memory System",
            section: "Memory System",
            valueText: "on",
            displayValueText: "Enabled",
            source: "project",
            sourceBadge: "P",
            editKind: "toggle",
            editKindLabel: "on/off",
            options: ["Disabled", "Enabled"],
            description: "Master toggle for the memory system.",
            summary: "Master toggle for the memory system.",
            impactText: "Disabling this turns off correction recall, learnings, ratchet checks, and graph context.",
            effectiveValueText: "on",
            effectiveDisplayValueText: "Enabled",
          },
          {
            keyPath: "memory.corrections.matching",
            label: "Trigger Matching",
            section: "Corrections",
            valueText: "keyword",
            displayValueText: "keyword",
            source: "global",
            sourceBadge: "G",
            editKind: "enum",
            editKindLabel: "choice",
            options: ["keyword", "ast-grep", "both"],
            description: "How Maestro matches saved corrections to the current task.",
            summary: "How Maestro matches saved corrections to the current task.",
            impactText: "Broader matching recalls more rules; narrower matching reduces noise.",
            effectiveValueText: "keyword",
            effectiveDisplayValueText: "keyword",
          },
          {
            keyPath: "memory.corrections.auto_capture",
            label: "Auto Capture",
            section: "Corrections",
            valueText: "prompt",
            displayValueText: "prompt",
            source: "global",
            sourceBadge: "G",
            editKind: "enum",
            editKindLabel: "choice",
            options: ["prompt", "auto", "off"],
            description: "When Maestro should capture corrections automatically.",
            summary: "When Maestro should capture corrections automatically.",
            impactText: "Prompt is safer; auto is faster; off requires explicit capture commands.",
            effectiveValueText: "prompt",
            effectiveDisplayValueText: "prompt",
          },
          {
            keyPath: "memory.learnings.compile_threshold",
            label: "Compile Threshold",
            section: "Learnings",
            valueText: "8",
            displayValueText: "8 entries",
            source: "project",
            sourceBadge: "P",
            editKind: "number-preset",
            editKindLabel: "number",
            options: ["3", "5", "8"],
            description: "How many raw learning entries should accumulate before compilation is suggested.",
            summary: "How many raw learning entries should accumulate before compilation is suggested.",
            impactText: "Lower values compile sooner; higher values keep more raw history around.",
            effectiveValueText: "8",
            effectiveDisplayValueText: "8 entries",
          },
          {
            keyPath: "memory.ratchet.enforcement",
            label: "Enforcement",
            section: "Ratchet",
            valueText: "warn",
            displayValueText: "warn",
            source: "default",
            sourceBadge: "D",
            editKind: "enum",
            editKindLabel: "choice",
            options: ["warn", "block"],
            description: "How ratchet failures are handled.",
            summary: "How ratchet failures are handled.",
            impactText: "Warn keeps the run moving; block stops progress until the regression is fixed.",
            effectiveValueText: "warn",
            effectiveDisplayValueText: "warn",
          },
          {
            keyPath: "memory.graph.enabled",
            label: "Project Graph",
            section: "Project Graph",
            valueText: "on",
            displayValueText: "Enabled",
            source: "default",
            sourceBadge: "D",
            editKind: "toggle",
            editKindLabel: "on/off",
            options: ["Disabled", "Enabled"],
            description: "Enable cross-project relationship context.",
            summary: "Enable cross-project relationship context.",
            impactText: "Turning this off removes project-link context from memory and TUI graph views.",
            effectiveValueText: "on",
            effectiveDisplayValueText: "Enabled",
          },
        ],
      },
      hasProjectConfig: true,
      hasGlobalConfig: true,
      projectPath: ".maestro/config.yaml",
      globalPath: "~/.maestro/config.yaml",
      errors: [],
    },
    progressLog: [],
    canPause: false,
    canResume: false,
    memory: {
        stats: {
          corrections: { total: 1, hard: 1, soft: 0 },
          learnings: { rawCount: 2, compiledAt: "2026-04-06T01:00:00.000Z", staleDays: 0 },
          ratchet: { assertions: 1, lastResult: "pass" },
          graph: { projects: 4, links: 3 },
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
          relationships: [
            {
              direction: "outgoing",
              project: { name: "maestro-web", path: "/tmp/maestro-web", role: "frontend" },
              edge: { from: "maestro", to: "maestro-web", relation: "exposes", detail: "mcp-tools" },
            },
            {
              direction: "incoming",
              project: { name: "maestro-api", path: "/tmp/maestro-api", role: "api" },
              edge: { from: "maestro-api", to: "maestro", relation: "consumes", detail: "rest-api" },
            },
            {
              direction: "incoming",
              project: { name: "shared-types", path: "/tmp/shared-types", role: "library" },
              edge: { from: "shared-types", to: "maestro", relation: "shared-types", detail: "types/mission.ts, types/config.ts" },
            },
          ],
          totalProjects: 4,
          totalEdges: 3,
        },
    },
      memoryStats: {
        corrections: { total: 1, hard: 1, soft: 0 },
        learnings: { rawCount: 2, compiledAt: "2026-04-06T01:00:00.000Z", staleDays: 0 },
        ratchet: { assertions: 1, lastResult: "pass" },
        graph: { projects: 4, links: 3 },
      },
    home: null,
  };
}

describe("memory modal", () => {
  it("renders an overview dashboard with memory cards and summaries", () => {
    const state = createInitialState(makeSnapshot());
    const memoryState = reduce(state, { type: "open-memory" });
    const modal = buildModalOptions(memoryState);

    expect(modal?.mode).toBe("info");
    if (!modal || modal.mode !== "info") return;
    expect(modal.eyebrow).toContain("[overview]");
    expect(modal.items.some((item) => item.text.includes("Corrections: 1"))).toBe(true);
    expect(modal.items.some((item) => item.text.includes("Learnings: 2 raw"))).toBe(true);
    expect(modal.items.some((item) => item.text.includes("Ratchet: 1"))).toBe(true);
    expect(modal.items.some((item) => item.text.includes("Last compiled: 2026-04-06 01:00"))).toBe(true);
    expect(modal.items.some((item) => item.text.includes("View All -->"))).toBe(true);
  });

  it("cycles tabs through config and resets the selected item index", () => {
    const state = reduce(createInitialState(makeSnapshot()), { type: "open-memory" });
    const correctionsState = reduce(state, { type: "memory-next-tab" });
    const learningsState = reduce(correctionsState, { type: "memory-next-tab" });
    const ratchetState = reduce(learningsState, { type: "memory-next-tab" });
    const configState = reduce(
      { ...ratchetState, modal: ratchetState.modal.kind === "memory" ? { ...ratchetState.modal, selectedItemIndex: 3 } : ratchetState.modal },
      { type: "memory-next-tab" },
    );
    const wrappedState = reduce(state, { type: "memory-prev-tab" });

    expect(correctionsState.modal.kind).toBe("memory");
    if (correctionsState.modal.kind === "memory") {
      expect(correctionsState.modal.tab).toBe("corrections");
    }
    if (configState.modal.kind === "memory") {
      expect(configState.modal.tab).toBe("config");
      expect(configState.modal.selectedItemIndex).toBe(0);
    }
    if (wrappedState.modal.kind === "memory") {
      expect(wrappedState.modal.tab).toBe("config");
      expect(wrappedState.modal.selectedItemIndex).toBe(0);
    }
  });

  it("renders correction detail, a learning activity timeline, and project graph impact detail", () => {
    const state = createInitialState(makeSnapshot());
    const correctionState = reduce(reduce(state, { type: "open-memory" }), { type: "memory-next-tab" });
    const memoryModal = buildModalOptions(correctionState);

    expect(memoryModal?.mode).toBe("split");
    if (!memoryModal || memoryModal.mode !== "split") return;
    expect(memoryModal.items[0]?.label).toContain("Use bun, not npm");
    expect(memoryModal.detailItems.some((item) => item.text.includes("Plan verification"))).toBe(true);
    expect(memoryModal.detailItems.some((item) => item.text.includes("package, install"))).toBe(true);
    expect(memoryModal.detailItems.some((item) => item.text.includes("HARD"))).toBe(true);

    const learningsState = reduce(correctionState, { type: "memory-next-tab" });
    const learningsModal = buildModalOptions(learningsState);

      expect(learningsModal?.mode).toBe("info");
      if (!learningsModal || learningsModal.mode !== "info") return;
      expect(learningsModal.items.some((item) => item.text.includes("Learning Activity"))).toBe(true);
      expect(learningsModal.items.some((item) => item.text.includes("Corrections captured:"))).toBe(true);
      expect(learningsModal.items.some((item) => item.text.includes("Next: ~8 more entries"))).toBe(true);

    const graphState = reduce(state, { type: "open-graph" });
    const graphModal = buildModalOptions(graphState);

    expect(graphModal?.mode).toBe("split");
    if (!graphModal || graphModal.mode !== "split") return;
    expect(graphModal.items[0]?.label).toContain("maestro");
    expect(graphModal.detailItems.some((item) => item.text.includes("Current project: maestro (cli)"))).toBe(true);
    expect(graphModal.detailItems.some((item) => item.text.includes("mcp-tools"))).toBe(true);
    expect(graphModal.detailItems.some((item) => item.text.includes("impact"))).toBe(true);
  });

  it("renders a standalone memory config tab from the config inspector memory rows", () => {
    const state = createInitialState(makeSnapshot());
    const memoryState = reduce(state, { type: "open-memory" });
    const correctionsState = reduce(memoryState, { type: "memory-next-tab" });
    const learningsState = reduce(correctionsState, { type: "memory-next-tab" });
    const ratchetState = reduce(learningsState, { type: "memory-next-tab" });
    const configState = reduce(ratchetState, { type: "memory-next-tab" });
    const modal = buildModalOptions(configState);

    expect(modal?.mode).toBe("info");
    if (!modal || modal.mode !== "info") return;
    expect(modal.eyebrow).toContain("[config]");
    expect(modal.title).toBe("Memory System");
    expect(modal.items.some((item) => item.text.includes("Memory System"))).toBe(true);
      expect(modal.items.some((item) => item.text.includes("Trigger Matching"))).toBe(true);
      expect(modal.items.some((item) => item.text.includes("Compile Threshold"))).toBe(true);
      expect(modal.items.some((item) => item.text.includes("Project Graph"))).toBe(true);
      expect(modal.items.some((item) => item.text.includes("Read-only summary"))).toBe(true);
      expect(modal.items.some((item) => item.text.includes("[ Save ]"))).toBe(false);
    });

  it("keeps learning activity non-selectable and lets graph navigation reach related projects", () => {
    const state = createInitialState(makeSnapshot());
    const learningsState = reduce(
      reduce(reduce(state, { type: "open-memory" }), { type: "memory-next-tab" }),
      { type: "memory-next-tab" },
    );
    const movedLearningsState = reduce(learningsState, { type: "navigate", direction: "down" });

    expect(movedLearningsState.modal.kind).toBe("memory");
    if (movedLearningsState.modal.kind === "memory") {
      expect(movedLearningsState.modal.tab).toBe("learnings");
      expect(movedLearningsState.modal.selectedItemIndex).toBe(0);
    }

    const graphState = reduce(state, { type: "open-graph" });
    const movedGraphState = reduce(graphState, { type: "navigate", direction: "down" });
    const graphModal = buildModalOptions(movedGraphState);

    expect(movedGraphState.modal.kind).toBe("graph");
    if (movedGraphState.modal.kind === "graph") {
      expect(movedGraphState.modal.selectedItemIndex).toBe(1);
    }
    expect(graphModal?.mode).toBe("split");
    if (!graphModal || graphModal.mode !== "split") return;
    expect(graphModal.items[1]?.label).toContain("maestro-web");
    expect(graphModal.selectedIndex).toBe(1);
  });
});
