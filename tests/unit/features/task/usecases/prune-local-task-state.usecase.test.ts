import { describe, expect, it } from "bun:test";
import { pruneLocalTaskState } from "@/features/task/usecases/prune-local-task-state.usecase.js";
import type { CandidateStorePort } from "@/features/task/ports/candidate-store.port.js";
import type { TaskContinuationStorePort } from "@/features/task/ports/task-continuation-store.port.js";
import type { TaskCandidate } from "@/features/task/domain/task-candidate.js";
import type { TaskContinuationSummary } from "@/features/task/domain/task-continuation-types.js";

interface CandidateState {
  readonly items: TaskCandidate[];
  readonly deletes: string[];
}

interface ContinuationState {
  readonly completed: TaskContinuationSummary[];
  readonly deletes: string[];
  readonly activeDeletes: string[];
}

function makeCandidate(id: string, capturedAt: string): TaskCandidate {
  return {
    id,
    sourceTaskId: id,
    sourceType: "task-close",
    title: `title ${id}`,
    reason: `reason ${id}`,
    keywords: [id],
    capturedAt,
  };
}

function makeCompletedSummary(taskId: string, lastActiveAt: string): TaskContinuationSummary {
  return {
    taskId,
    status: "completed",
    lastActiveAt,
    currentState: `${taskId} current state`,
    nextAction: `${taskId} next action`,
    keyDecisions: [],
  };
}

function createCandidateStore(state: CandidateState): CandidateStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };
  return {
    create: unused,
    all: async () => state.items.slice(),
    delete: async (id: string) => {
      state.deletes.push(id);
      const index = state.items.findIndex((c) => c.id === id);
      if (index >= 0) state.items.splice(index, 1);
    },
  };
}

function createContinuationStore(state: ContinuationState): TaskContinuationStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };
  return {
    getActive: unused,
    getCompleted: unused,
    listActive: unused,
    listCompleted: async () => state.completed.slice(),
    upsertActive: unused,
    archiveCompleted: unused,
    reopen: unused,
    delete: async (taskId: string) => {
      state.activeDeletes.push(taskId);
    },
    deleteCompleted: async (taskId: string) => {
      state.deletes.push(taskId);
      const index = state.completed.findIndex((s) => s.taskId === taskId);
      if (index >= 0) state.completed.splice(index, 1);
    },
  };
}

describe("pruneLocalTaskState", () => {
  it("returns zero counts for an empty store", async () => {
    const candidateState: CandidateState = { items: [], deletes: [] };
    const continuationState: ContinuationState = { completed: [], deletes: [], activeDeletes: [] };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 500, kinds: "both", all: false, dryRun: false },
    );

    expect(report.candidates.purged).toBe(0);
    expect(report.candidates.kept).toBe(0);
    expect(report.continuations.purged).toBe(0);
    expect(report.continuations.kept).toBe(0);
    expect(candidateState.deletes).toEqual([]);
    expect(continuationState.deletes).toEqual([]);
  });

  it("keeps everything when count is under the cap", async () => {
    const candidateState: CandidateState = {
      items: [
        makeCandidate("tsk-old", "2026-04-10T00:00:00.000Z"),
        makeCandidate("tsk-new", "2026-04-20T00:00:00.000Z"),
      ],
      deletes: [],
    };
    const continuationState: ContinuationState = { completed: [], deletes: [], activeDeletes: [] };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 500, kinds: "both", all: false, dryRun: false },
    );

    expect(report.candidates).toEqual({
      purged: 0,
      kept: 2,
      oldestKeptAt: "2026-04-10T00:00:00.000Z",
    });
    expect(candidateState.deletes).toEqual([]);
  });

  it("purges oldest candidates past --keep cap, newest kept first", async () => {
    const items = Array.from({ length: 5 }, (_, i) =>
      makeCandidate(`tsk-${i}00000`, `2026-04-${String(10 + i).padStart(2, "0")}T00:00:00.000Z`),
    );
    const candidateState: CandidateState = { items: items.slice(), deletes: [] };
    const continuationState: ContinuationState = { completed: [], deletes: [], activeDeletes: [] };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 2, kinds: "both", all: false, dryRun: false },
    );

    expect(report.candidates.kept).toBe(2);
    expect(report.candidates.purged).toBe(3);
    expect(report.candidates.oldestKeptAt).toBe("2026-04-13T00:00:00.000Z");
    expect(report.candidates.newestPurgedAt).toBe("2026-04-12T00:00:00.000Z");
    expect(candidateState.deletes).toEqual([
      "tsk-200000",
      "tsk-100000",
      "tsk-000000",
    ]);
    expect(candidateState.items.map((c) => c.id).sort()).toEqual([
      "tsk-300000",
      "tsk-400000",
    ]);
  });

  it("purges completed continuations past --keep cap", async () => {
    const summaries = Array.from({ length: 4 }, (_, i) =>
      makeCompletedSummary(
        `tsk-c${i}00000`,
        `2026-04-${String(10 + i).padStart(2, "0")}T00:00:00.000Z`,
      ),
    );
    const candidateState: CandidateState = { items: [], deletes: [] };
    const continuationState: ContinuationState = {
      completed: summaries.slice(),
      deletes: [],
      activeDeletes: [],
    };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 1, kinds: "both", all: false, dryRun: false },
    );

    expect(report.continuations.kept).toBe(1);
    expect(report.continuations.purged).toBe(3);
    expect(continuationState.deletes).toEqual([
      "tsk-c200000",
      "tsk-c100000",
      "tsk-c000000",
    ]);
    expect(continuationState.activeDeletes).toEqual([]);
  });

  it("respects --candidates-only by leaving continuations untouched", async () => {
    const candidateState: CandidateState = {
      items: [makeCandidate("tsk-a", "2026-04-20T00:00:00.000Z")],
      deletes: [],
    };
    const continuationState: ContinuationState = {
      completed: [makeCompletedSummary("tsk-b", "2026-04-20T00:00:00.000Z")],
      deletes: [],
      activeDeletes: [],
    };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 0, kinds: "candidates", all: false, dryRun: false },
    );

    expect(report.candidates.purged).toBe(1);
    expect(report.continuations).toEqual({ purged: 0, kept: 0 });
    expect(continuationState.deletes).toEqual([]);
  });

  it("respects --continuations-only by leaving candidates untouched", async () => {
    const candidateState: CandidateState = {
      items: [makeCandidate("tsk-a", "2026-04-20T00:00:00.000Z")],
      deletes: [],
    };
    const continuationState: ContinuationState = {
      completed: [makeCompletedSummary("tsk-b", "2026-04-20T00:00:00.000Z")],
      deletes: [],
      activeDeletes: [],
    };

    await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 0, kinds: "continuations", all: false, dryRun: false },
    );

    expect(candidateState.deletes).toEqual([]);
    expect(continuationState.deletes).toEqual(["tsk-b"]);
  });

  it("purges everything when --all is set regardless of --keep", async () => {
    const candidateState: CandidateState = {
      items: [
        makeCandidate("tsk-old", "2026-04-10T00:00:00.000Z"),
        makeCandidate("tsk-new", "2026-04-20T00:00:00.000Z"),
      ],
      deletes: [],
    };
    const continuationState: ContinuationState = {
      completed: [makeCompletedSummary("tsk-c", "2026-04-20T00:00:00.000Z")],
      deletes: [],
      activeDeletes: [],
    };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 999, kinds: "both", all: true, dryRun: false },
    );

    expect(report.candidates.purged).toBe(2);
    expect(report.candidates.kept).toBe(0);
    expect(report.continuations.purged).toBe(1);
    expect(candidateState.deletes.sort()).toEqual(["tsk-new", "tsk-old"]);
    expect(continuationState.deletes).toEqual(["tsk-c"]);
  });

  it("reports counts but performs no deletes on --dry-run", async () => {
    const candidateState: CandidateState = {
      items: [
        makeCandidate("tsk-old", "2026-04-10T00:00:00.000Z"),
        makeCandidate("tsk-new", "2026-04-20T00:00:00.000Z"),
      ],
      deletes: [],
    };
    const continuationState: ContinuationState = {
      completed: [
        makeCompletedSummary("tsk-c1", "2026-04-10T00:00:00.000Z"),
        makeCompletedSummary("tsk-c2", "2026-04-20T00:00:00.000Z"),
      ],
      deletes: [],
      activeDeletes: [],
    };

    const report = await pruneLocalTaskState(
      {
        candidateStore: createCandidateStore(candidateState),
        continuationStore: createContinuationStore(continuationState),
      },
      { keep: 1, kinds: "both", all: false, dryRun: true },
    );

    expect(report.dryRun).toBe(true);
    expect(report.candidates.purged).toBe(1);
    expect(report.continuations.purged).toBe(1);
    expect(candidateState.deletes).toEqual([]);
    expect(continuationState.deletes).toEqual([]);
    expect(candidateState.items.length).toBe(2);
    expect(continuationState.completed.length).toBe(2);
  });
});
