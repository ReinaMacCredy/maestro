import { describe, expect, it } from "bun:test";
import {
  buildAgentGrid,
  buildDispatchQueue,
  buildEventStream,
  buildHomeSnapshot,
  buildPrincipleEffectivenessRows,
  buildReplyInbox,
  buildTaskBoard,
  buildTimelineMilestones,
} from "@/tui/state/snapshot.js";
import { buildModalOptions } from "@/tui/app/modal-builders.js";
import { createInitialState, reduce } from "@/tui/state/reducer.js";
import type { Feature, Milestone } from "@/shared/domain/legacy-mission";
import type { Principle, PrincipleOutcomeRecord, PrincipleStorePort } from "@/features/principle";
import type { MissionControlEvent, MissionControlSnapshot } from "@/tui/state/types.js";
import type { Task, TaskQueryPort } from "@/shared/domain/task";
import type { AgentReply } from "@/features/reply";
import type { EvidenceSummary } from "@/tui/state/screen-types.js";
import { mockConfig, mockGit } from "../../../helpers/mocks.js";

function makeFeature(overrides: Partial<Feature> & { id: string }): Feature {
  return {
    missionId: "m-1",
    milestoneId: "ms-1",
    status: "pending",
    title: overrides.id,
    description: "",
    agentType: "codex",
    verificationSteps: [],
    dependsOn: [],
    fulfills: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function makeMilestone(overrides: Partial<Milestone> & { id: string }): Milestone {
  return {
    title: overrides.id,
    description: "",
    order: 0,
    featureIds: [],
    ...overrides,
  };
}

function makeTaskQueryStore(tasks: readonly Task[]): TaskQueryPort {
  return {
    get: async (id: string) => tasks.find((task) => task.id === id),
    all: async () => tasks,
  };
}

// ---------------------------------------------------------------------------
// buildAgentGrid
// ---------------------------------------------------------------------------

describe("buildAgentGrid", () => {
  it("groups features by agentType and infers status", () => {
    const features = [
      makeFeature({ id: "f1", agentType: "codex", status: "assigned", updatedAt: new Date().toISOString() }),
      makeFeature({ id: "f2", agentType: "codex", status: "pending" }),
      makeFeature({ id: "f3", agentType: "claude-code", status: "done" }),
      makeFeature({ id: "f4", agentType: "claude-code", status: "done" }),
    ];
    const grid = buildAgentGrid(features);
    expect(grid).toHaveLength(2);

    const codex = grid.find((r) => r.agentType === "codex");
    expect(codex?.status).toBe("active");
    expect(codex?.featureCount).toBe(2);
    expect(codex?.completedCount).toBe(0);
    expect(codex?.activeFeatureId).toBe("f1");

    const claude = grid.find((r) => r.agentType === "claude-code");
    expect(claude?.status).toBe("completed");
    expect(claude?.completedCount).toBe(2);
  });

  it("sorts active before waiting before completed", () => {
    const features = [
      makeFeature({ id: "f1", agentType: "a-completed", status: "done" }),
      makeFeature({ id: "f2", agentType: "b-active", status: "in-progress", updatedAt: new Date().toISOString() }),
      makeFeature({ id: "f3", agentType: "c-waiting", status: "review" }),
    ];
    const grid = buildAgentGrid(features);
    expect(grid[0]!.agentType).toBe("b-active");
    expect(grid[1]!.agentType).toBe("c-waiting");
    expect(grid[2]!.agentType).toBe("a-completed");
  });

  it("does not mark completed agents waiting for another agent's pending work", () => {
    const features = [
      makeFeature({ id: "f1", agentType: "claude-code", status: "done" }),
      makeFeature({ id: "f2", agentType: "claude-code", status: "done" }),
      makeFeature({ id: "f3", agentType: "codex", status: "pending" }),
    ];
    const grid = buildAgentGrid(features);

    expect(grid.find((row) => row.agentType === "claude-code")?.status).toBe("completed");
    expect(grid.find((row) => row.agentType === "codex")?.status).toBe("waiting");
  });
});

// ---------------------------------------------------------------------------
// buildDispatchQueue
// ---------------------------------------------------------------------------

describe("buildDispatchQueue", () => {
  it("includes only ready (pending + deps done) features", () => {
    const features = [
      makeFeature({ id: "f1", status: "pending", dependsOn: ["f2"], milestoneId: "ms-1" }),
      makeFeature({ id: "f2", status: "done", milestoneId: "ms-1" }),
      makeFeature({ id: "f3", status: "pending", dependsOn: [], milestoneId: "ms-2" }),
      makeFeature({ id: "f4", status: "assigned", dependsOn: [], milestoneId: "ms-1" }),
    ];
    const milestones = [
      makeMilestone({ id: "ms-1", order: 1 }),
      makeMilestone({ id: "ms-2", order: 2 }),
    ];
    const queue = buildDispatchQueue(features, milestones);
    // f1 ready (dep f2 is done), f3 ready (no deps), f4 excluded (not pending)
    expect(queue).toHaveLength(2);
    expect(queue.map((q) => q.featureId)).toEqual(["f1", "f3"]);
  });

  it("excludes features with unfinished dependencies", () => {
    const features = [
      makeFeature({ id: "f1", status: "pending", dependsOn: ["f2"] }),
      makeFeature({ id: "f2", status: "in-progress" }),
    ];
    const queue = buildDispatchQueue(features, []);
    expect(queue).toHaveLength(0);
  });

  it("sorts by milestone order ascending", () => {
    const features = [
      makeFeature({ id: "f1", status: "pending", milestoneId: "ms-2" }),
      makeFeature({ id: "f2", status: "pending", milestoneId: "ms-1" }),
    ];
    const milestones = [
      makeMilestone({ id: "ms-1", order: 1 }),
      makeMilestone({ id: "ms-2", order: 2 }),
    ];
    const queue = buildDispatchQueue(features, milestones);
    expect(queue[0]!.featureId).toBe("f2");
    expect(queue[1]!.featureId).toBe("f1");
  });
});

// ---------------------------------------------------------------------------
// buildEventStream
// ---------------------------------------------------------------------------

describe("buildEventStream", () => {
  it("merges progress log and reply events, sorted descending", () => {
    const log: MissionControlEvent[] = [
      { timestamp: "2026-01-01T00:00:00Z", relativeMs: 0, kind: "mission", title: "Created" },
      { timestamp: "2026-01-01T01:00:00Z", relativeMs: 3600000, kind: "feature", title: "f1 assigned" },
    ];
    const replies = [
      makeReply({ featureId: "f1", outcome: "completed", writtenAt: "2026-01-01T00:30:00Z" }),
    ];
    const stream = buildEventStream(log, replies);

    expect(stream.length).toBeGreaterThanOrEqual(3);
    expect(stream[0]!.kind).toBe("feature");
    expect(stream.find((entry) => entry.kind === "reply")?.timestamp).toBe("2026-01-01T00:30:00Z");
  });

  it("caps at 200 entries", () => {
    const log: MissionControlEvent[] = Array.from({ length: 250 }, (_, i) => ({
      timestamp: new Date(Date.now() - i * 1000).toISOString(),
      relativeMs: i * 1000,
      kind: "feature" as const,
      title: `Event ${i}`,
    }));
    const stream = buildEventStream(log);
    expect(stream).toHaveLength(200);
  });
});

// ---------------------------------------------------------------------------
// buildTaskBoard
// ---------------------------------------------------------------------------

describe("buildTaskBoard", () => {
  it("returns null when no store provided", async () => {
    const result = await buildTaskBoard(undefined);
    expect(result).toBeNull();
  });

  it("groups tasks into status columns sorted by priority", async () => {
      const store = makeTaskQueryStore([
        { id: "t1", title: "A", description: "d", type: "task", priority: 2, status: "pending", labels: [], blocks: [], blockedBy: [], createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
        { id: "t2", title: "B", description: "d", type: "task", priority: 0, status: "pending", labels: [], blocks: [], blockedBy: ["t1"], createdAt: "2026-01-01T00:00:01Z", updatedAt: "2026-01-01T00:00:01Z" },
        { id: "t3", title: "C", description: "d", type: "task", priority: 1, status: "in_progress", labels: ["urgent"], blocks: [], blockedBy: [], createdAt: "2026-01-01T00:00:02Z", updatedAt: "2026-01-01T00:00:02Z" },
        { id: "t4", title: "D", description: "d", type: "task", priority: 3, status: "completed", labels: [], blocks: [], blockedBy: [], createdAt: "2026-01-01T00:00:03Z", updatedAt: "2026-01-01T00:00:03Z" },
      ]);
    const board = await buildTaskBoard(store);
    expect(board).not.toBeNull();
    expect(board!.totalCount).toBe(4);

    // Pending column: t2 (priority 0) before t1 (priority 2)
    expect(board!.columns.pending).toHaveLength(2);
    expect(board!.columns.pending[0]!.id).toBe("t2");
    expect(board!.columns.pending[0]!.blockedByCount).toBe(1);
    expect(board!.columns.pending[1]!.id).toBe("t1");

    // In-progress column
    expect(board!.columns.in_progress).toHaveLength(1);
    expect(board!.columns.in_progress[0]!.labels).toEqual(["urgent"]);

    // Completed column
    expect(board!.columns.completed).toHaveLength(1);

    // Remaining column
    expect(board!.columns.in_progress).toHaveLength(1);
  });

  it("returns null when store has no tasks", async () => {
      const store = makeTaskQueryStore([]);
    const result = await buildTaskBoard(store);
    expect(result).toBeNull();
  });

  it("surfaces task store errors instead of hiding corruption", async () => {
    const store = {
      get: async () => undefined,
      all: async () => {
        throw new Error("corrupted tasks");
      },
    };

    await expect(buildTaskBoard(store)).rejects.toThrow("corrupted tasks");
  });
});

// ---------------------------------------------------------------------------
// buildTimelineMilestones
// ---------------------------------------------------------------------------

describe("buildTimelineMilestones", () => {
  it("computes progress percentage per milestone", () => {
    const milestones = [
      makeMilestone({ id: "ms-1", order: 1 }),
      makeMilestone({ id: "ms-2", order: 2 }),
    ];
    const features = [
      makeFeature({ id: "f1", milestoneId: "ms-1", status: "done" }),
      makeFeature({ id: "f2", milestoneId: "ms-1", status: "done" }),
      makeFeature({ id: "f3", milestoneId: "ms-1", status: "pending" }),
      makeFeature({ id: "f4", milestoneId: "ms-1", status: "assigned" }),
      makeFeature({ id: "f5", milestoneId: "ms-2", status: "pending" }),
    ];
    const timeline = buildTimelineMilestones(milestones, features);
    expect(timeline).toHaveLength(2);

    // ms-1: 2 done out of 4 = 50%
    expect(timeline[0]!.progressPct).toBe(50);
    expect(timeline[0]!.features).toHaveLength(4);

    // ms-2: 0 done out of 1 = 0%
    expect(timeline[1]!.progressPct).toBe(0);
    expect(timeline[1]!.features).toHaveLength(1);
  });

  it("handles milestone with no features (0%)", () => {
    const milestones = [makeMilestone({ id: "ms-empty", order: 1 })];
    const timeline = buildTimelineMilestones(milestones, []);
    expect(timeline[0]!.progressPct).toBe(0);
    expect(timeline[0]!.features).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// buildReplyInbox
// ---------------------------------------------------------------------------

function makeReply(overrides: Partial<AgentReply> & { featureId: string }): AgentReply {
  return {
    missionId: "2026-04-13-001",
    outcome: "completed",
    writtenAt: "2026-04-13T00:00:00.000Z",
    writtenBy: "human",
    ...overrides,
  };
}

describe("buildReplyInbox", () => {
  it("enriches entries with matching feature title and status", () => {
    const features = [
      makeFeature({ id: "f1", title: "Feature One", status: "done" }),
      makeFeature({ id: "f2", title: "Feature Two", status: "review" }),
    ];
    const replies = [
      makeReply({ featureId: "f1", outcome: "completed", writtenAt: "2026-04-13T01:00:00.000Z" }),
      makeReply({ featureId: "f2", outcome: "kicked-back", writtenAt: "2026-04-13T02:00:00.000Z" }),
    ];

    const inbox = buildReplyInbox(features, replies);
    // Newest first
    expect(inbox.map((e) => e.featureId)).toEqual(["f2", "f1"]);
    expect(inbox[0]!.featureTitle).toBe("Feature Two");
    expect(inbox[0]!.featureStatus).toBe("review");
    expect(inbox[0]!.pending).toBe(true); // kicked-back expects pending, feature is review
    expect(inbox[1]!.featureStatus).toBe("done");
    expect(inbox[1]!.pending).toBe(false); // completed, feature is done -- settled
  });

  it("marks reply as pending when no feature match exists", () => {
    const inbox = buildReplyInbox([], [makeReply({ featureId: "f-missing" })]);
    expect(inbox[0]!.pending).toBe(true);
    expect(inbox[0]!.featureTitle).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// buildEventStream with reply entries
// ---------------------------------------------------------------------------

describe("buildEventStream with replies", () => {
  it("merges reply entries with kind='reply'", () => {
    const replies = [
      makeReply({ featureId: "f1", outcome: "completed", writtenAt: "2026-04-13T01:00:00.000Z" }),
      makeReply({ featureId: "f2", outcome: "kicked-back", writtenAt: "2026-04-13T02:00:00.000Z" }),
    ];

    const entries = buildEventStream([], replies);
    expect(entries.filter((e) => e.kind === "reply")).toHaveLength(2);
    const kickback = entries.find((e) => e.title.includes("kicked back"));
    expect(kickback).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// buildPrincipleEffectivenessRows
// ---------------------------------------------------------------------------

function makePrinciple(id: string, name: string = id): Principle {
  return {
    id,
    name,
    source: "custom",
    rule: "r",
    profiles: ["implementation"],
    mode: "gate",
    gateField: "assumptions",
    gateCheck: "array_min_length:1",
  };
}

function makeOutcome(
  principleId: string,
  handoffId: string,
  outcome: "pending" | "helpful" | "unhelpful",
  recordedAt: string,
): PrincipleOutcomeRecord {
  return { principleId, handoffId, outcome, recordedAt };
}

describe("buildPrincipleEffectivenessRows", () => {
  it("returns empty when no principles exist", () => {
    const rows = buildPrincipleEffectivenessRows([], []);
    expect(rows).toEqual([]);
  });

  it("sorts by effectiveness ascending (worst first) and flags low sample", () => {
    const principles = [
      makePrinciple("p-good", "Good"),
      makePrinciple("p-bad", "Bad"),
      makePrinciple("p-new", "New"),
    ];
    const outcomes = [
      makeOutcome("p-good", "h1", "helpful", "2026-04-13T00:00:00Z"),
      makeOutcome("p-good", "h2", "helpful", "2026-04-13T01:00:00Z"),
      makeOutcome("p-good", "h3", "helpful", "2026-04-13T02:00:00Z"),
      makeOutcome("p-bad", "h1", "unhelpful", "2026-04-13T00:00:00Z"),
      makeOutcome("p-bad", "h2", "helpful", "2026-04-13T01:00:00Z"),
      makeOutcome("p-bad", "h3", "unhelpful", "2026-04-13T02:00:00Z"),
      makeOutcome("p-new", "h4", "helpful", "2026-04-13T03:00:00Z"),
    ];

    const rows = buildPrincipleEffectivenessRows(principles, outcomes);
    // p-bad: 33%, p-good: 100%, p-new: undecided+low -> sorts last
    expect(rows[0]!.id).toBe("p-bad");
    expect(rows[0]!.effectivenessPct).toBe(33);
    expect(rows[0]!.lowSample).toBe(false);
    expect(rows[1]!.id).toBe("p-good");
    expect(rows[1]!.effectivenessPct).toBe(100);
    expect(rows[2]!.id).toBe("p-new");
    expect(rows[2]!.lowSample).toBe(true);
  });

  it("emits unhelpful kickback examples (most recent first)", () => {
    const principles = [makePrinciple("p-1")];
    const outcomes = [
      makeOutcome("p-1", "h-1", "unhelpful", "2026-04-13T00:00:00Z"),
      makeOutcome("p-1", "h-2", "unhelpful", "2026-04-13T01:00:00Z"),
      makeOutcome("p-1", "h-3", "helpful", "2026-04-13T02:00:00Z"),
    ];

    const rows = buildPrincipleEffectivenessRows(principles, outcomes);
    const row = rows[0]!;
    expect(row.recentKickbackExamples.length).toBe(2);
    expect(row.recentKickbackExamples[0]).toContain("h-2"); // newest first
    expect(row.recentKickbackExamples[1]).toContain("h-1");
  });
});

describe("buildHomeSnapshot principle effectiveness", () => {
  it("rolls up principle outcomes without handoff scoping", async () => {
    const currentProjectRoot = "/tmp/maestro-project-a";
    const principles = [makePrinciple("p-1", "Preserve ownership")];
    const outcomes = [
      makeOutcome("p-1", "h-local", "unhelpful", "2026-04-13T01:00:00Z"),
    ];
    const principleStore = {
      list: async () => principles,
      listOutcomes: async () => outcomes,
    } as unknown as PrincipleStorePort;

    const snapshot = await buildHomeSnapshot(
      {
        config: mockConfig(),
        git: mockGit(),
        cwd: currentProjectRoot,
        principleStore,
      },
      { includeReplies: true },
    );

    const row = snapshot.principleEffectiveness?.[0];
    expect(row).toMatchObject({
      id: "p-1",
      unhelpful: 1,
      total: 1,
    });
  });
});

// ---------------------------------------------------------------------------
// buildTaskBoardModal — Evidence section (via buildModalOptions)
// ---------------------------------------------------------------------------

function makeMinimalSnapshot(taskBoard: MissionControlSnapshot["taskBoard"]): MissionControlSnapshot {
  return {
    mode: "home",
    missionId: "",
    missionTitle: "",
    missionStatus: "draft",
    effectiveStatus: "draft",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 0, active: 0 },
    statusProgress: { completed: 0, total: 0, inFlight: 0, blocked: 0, queued: 0, completionPct: 0 },
    tokenCounters: null,
    features: [],
    milestones: [],
    activeFeature: null,
    session: null,
    configSummary: null,
    progressLog: [],
    canPause: false,
    canResume: false,
    home: null,
    taskBoard,
  } as unknown as MissionControlSnapshot;
}

describe("buildTaskBoardModal — Evidence section", () => {
  it("renders Evidence section with rows when recentEvidence is non-empty", () => {
    const evidence: EvidenceSummary = {
      id: "evd-0000000000001-aaaaaa",
      kind: "command",
      witness_level: "agent-claimed-locally",
      created_at: "2026-05-03T00:00:01.000Z",
    };
    const snapshot = makeMinimalSnapshot({
      totalCount: 1,
      columns: {
        pending: [{
          id: "tsk-aaaaaa",
          title: "Test task",
          status: "pending",
          priority: 1,
          assignee: undefined,
          labels: [],
          blockedByCount: 0,
          evidenceCount: 1,
          recentEvidence: [evidence],
        }],
        in_progress: [],
        completed: [],
      },
    });

    const state = createInitialState(snapshot);
    const taskBoardState = reduce(state, { type: "open-task-board" });
    const modal = buildModalOptions(taskBoardState);

    expect(modal?.mode).toBe("split");
    if (!modal || modal.mode !== "split") return;

    const detailItems = modal.detailItems ?? [];
    const evidenceHeader = detailItems.find((item) => "section" in item && item.section === "Evidence");
    expect(evidenceHeader).toBeDefined();
    expect((evidenceHeader as { text: string }).text).toBe("Evidence: 1 recorded");

    const hasEvidenceRow = detailItems.some((item) =>
      "text" in item
      && item.text.includes("command")
      && item.text.includes("agent-claimed-locally")
      && item.text.includes("2026-05-03T00:00:01.000Z"),
    );
    expect(hasEvidenceRow).toBe(true);
  });

  it("renders (no evidence) when recentEvidence is empty", () => {
    const snapshot = makeMinimalSnapshot({
      totalCount: 1,
      columns: {
        pending: [{
          id: "tsk-aaaaaa",
          title: "Test task",
          status: "pending",
          priority: 1,
          assignee: undefined,
          labels: [],
          blockedByCount: 0,
          evidenceCount: 0,
          recentEvidence: [],
        }],
        in_progress: [],
        completed: [],
      },
    });

    const state = createInitialState(snapshot);
    const taskBoardState = reduce(state, { type: "open-task-board" });
    const modal = buildModalOptions(taskBoardState);

    expect(modal?.mode).toBe("split");
    if (!modal || modal.mode !== "split") return;

    const detailItems = modal.detailItems ?? [];
    const noEvidenceLine = detailItems.find((item) =>
      "text" in item && item.text === "(no evidence)",
    );
    expect(noEvidenceLine).toBeDefined();
  });
});
