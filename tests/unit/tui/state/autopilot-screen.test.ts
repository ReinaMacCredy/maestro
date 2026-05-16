import { describe, expect, it } from "bun:test";
import { buildAutopilotSnapshot } from "@/tui/state/autopilot-screen.js";
import type { AutopilotSnapshotDeps } from "@/tui/state/autopilot-screen.js";
import type { LegacyTask as Task } from "@/shared/domain/legacy-task";
import type { RunState } from "@/shared/domain/legacy-task/domain/run-state.js";
import type { Verdict } from "@/features/verdict";
import type { Contract } from "@/types/contract.js";
import { CONTRACT_SCHEMA_VERSION } from "@/shared/domain/legacy-task/domain/contract/contract-types.js";
import { mockContractStore } from "../../../helpers/mocks.js";

// ---- minimal fake factories ----

function makeTask(id: string, missionId: string = "msn-1"): Task {
  return {
    id,
    title: `Task ${id}`,
    type: "task",
    priority: 1,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    missionId,
    createdAt: "2026-05-04T00:00:00.000Z",
    updatedAt: "2026-05-04T00:00:00.000Z",
  };
}

function makeVerdict(taskId: string, decision: Verdict["decision"]): Verdict {
  return {
    schemaVersion: 1,
    id: `vrd-${taskId}`,
    taskId,
    contractVersion: 1,
    computedAt: "2026-05-04T17:30:00.000Z",
    decision,
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
  };
}

function makeRunState(taskId: string, retryCount = 2, wallClock = 180): RunState {
  return {
    schemaVersion: 1,
    taskId,
    retryCount,
    wallClockElapsedSeconds: wallClock,
    lastUpdatedAt: "2026-05-04T17:32:00.000Z",
  };
}

function makeContract(taskId: string, intent?: string, maxRetries?: number, maxWallClock?: number): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: `ctr-${taskId}`,
    taskId,
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-05-04T00:00:00.000Z",
    intent: intent ?? `Intent for ${taskId}`,
    scope: { filesExpected: [], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: { strict: false, overlapPolicy: "annotate", rebaseFallback: "best-effort", staleReclaimContractPolicy: "inherit" },
    ...(maxRetries !== undefined || maxWallClock !== undefined
      ? { costBudget: { maxRetries: maxRetries ?? 5, maxWallClockSeconds: maxWallClock ?? 3600 } }
      : {}),
  };
}

// ---- helper: build minimal deps ----

function makeDeps(opts: {
  tasks?: Task[];
  verdicts?: Record<string, Verdict>;
  runStates?: Record<string, RunState>;
  contracts?: Record<string, Contract>;
  missionId?: string;
}): AutopilotSnapshotDeps {
  const { tasks = [], verdicts = {}, runStates = {}, contracts = {} } = opts;

  return {
    taskStore: {
      get: async (id) => tasks.find((t) => t.id === id),
      all: async () => tasks,
    },
    verdictStore: {
      write: async () => {},
      readLatest: async (taskId) => verdicts[taskId],
      readVersion: async () => undefined,
      history: async () => [],
      findByTreeSha: async () => [],
    },
    runStateStore: {
      read: async (taskId) => runStates[taskId],
      write: async () => {},
      increment: async () => ({ schemaVersion: 1, taskId: "", retryCount: 0, wallClockElapsedSeconds: 0, lastUpdatedAt: "" }),
    },
    contractVersionStore: {
      write: async () => {},
      readCurrent: async (taskId) => contracts[taskId],
      readVersion: async () => undefined,
      history: async () => [],
    },
    contractStore: mockContractStore(),
  };
}

// ---- tests ----

describe("buildAutopilotSnapshot", () => {
  it("projects tasks with mixed verdicts (PASS, FAIL, HUMAN)", async () => {
    const missionId = "msn-1";
    const tasks = [
      makeTask("tsk-aaaa", missionId),
      makeTask("tsk-bbbb", missionId),
      makeTask("tsk-cccc", missionId),
    ];
    const verdicts: Record<string, Verdict> = {
      "tsk-aaaa": makeVerdict("tsk-aaaa", "PASS"),
      "tsk-bbbb": makeVerdict("tsk-bbbb", "FAIL"),
      "tsk-cccc": makeVerdict("tsk-cccc", "HUMAN"),
    };
    const runStates: Record<string, RunState> = {
      "tsk-aaaa": makeRunState("tsk-aaaa", 0, 12),
      "tsk-bbbb": makeRunState("tsk-bbbb", 2, 180),
      "tsk-cccc": makeRunState("tsk-cccc", 1, 60),
    };
    const contracts: Record<string, Contract> = {
      "tsk-aaaa": makeContract("tsk-aaaa", "Implement A", 5, 3600),
      "tsk-bbbb": makeContract("tsk-bbbb", "Implement B", 5, 3600),
      "tsk-cccc": makeContract("tsk-cccc", "Implement C", 5, 3600),
    };

    const deps = makeDeps({ tasks, verdicts, runStates, contracts });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    expect(snapshot.tasks).toHaveLength(3);

    const rowA = snapshot.tasks.find((r) => r.taskId === "tsk-aaaa")!;
    expect(rowA.latestVerdict?.decision).toBe("PASS");
    expect(rowA.retryCount).toBe(0);
    expect(rowA.wallClockElapsedSeconds).toBe(12);
    expect(rowA.maxRetries).toBe(5);
    expect(rowA.maxWallClockSeconds).toBe(3600);
    expect(rowA.intent).toBe("Implement A");

    const rowB = snapshot.tasks.find((r) => r.taskId === "tsk-bbbb")!;
    expect(rowB.latestVerdict?.decision).toBe("FAIL");
    expect(rowB.retryCount).toBe(2);

    const rowC = snapshot.tasks.find((r) => r.taskId === "tsk-cccc")!;
    expect(rowC.latestVerdict?.decision).toBe("HUMAN");
  });

  it("returns latestVerdict: undefined for task with no verdict", async () => {
    const missionId = "msn-2";
    const tasks = [makeTask("tsk-dddd", missionId)];
    const deps = makeDeps({ tasks });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    expect(snapshot.tasks).toHaveLength(1);
    const row = snapshot.tasks[0]!;
    expect(row.latestVerdict).toBeUndefined();
  });

  it("returns retryCount=0, wallClockElapsedSeconds=0, lastUpdatedAt=undefined for task with no run-state", async () => {
    const missionId = "msn-3";
    const tasks = [makeTask("tsk-eeee", missionId)];
    const deps = makeDeps({ tasks });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    const row = snapshot.tasks[0]!;
    expect(row.retryCount).toBe(0);
    expect(row.wallClockElapsedSeconds).toBe(0);
    expect(row.lastUpdatedAt).toBeUndefined();
  });

  it("returns maxRetries=undefined, maxWallClockSeconds=undefined for task with no contract.costBudget", async () => {
    const missionId = "msn-4";
    const tasks = [makeTask("tsk-ffff", missionId)];
    const contracts: Record<string, Contract> = {
      "tsk-ffff": makeContract("tsk-ffff", "No budget"),
      // no costBudget set
    };
    const deps = makeDeps({ tasks, contracts });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    const row = snapshot.tasks[0]!;
    expect(row.maxRetries).toBeUndefined();
    expect(row.maxWallClockSeconds).toBeUndefined();
  });

  it("uses contract.intent as intent when contract is available", async () => {
    const missionId = "msn-5";
    const tasks = [makeTask("tsk-gggg", missionId)];
    const contracts: Record<string, Contract> = {
      "tsk-gggg": makeContract("tsk-gggg", "Custom contract intent"),
    };
    const deps = makeDeps({ tasks, contracts });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    expect(snapshot.tasks[0]!.intent).toBe("Custom contract intent");
  });

  it("falls back to task.title as intent when no contract exists", async () => {
    const missionId = "msn-6";
    const tasks = [makeTask("tsk-hhhh", missionId)];
    const deps = makeDeps({ tasks });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    expect(snapshot.tasks[0]!.intent).toBe("Task tsk-hhhh");
  });

  it("only includes tasks for the given missionId", async () => {
    const tasks = [
      makeTask("tsk-iiii", "msn-target"),
      makeTask("tsk-jjjj", "msn-other"),
    ];
    const deps = makeDeps({ tasks });
    const snapshot = await buildAutopilotSnapshot(deps, "msn-target");

    expect(snapshot.tasks).toHaveLength(1);
    expect(snapshot.tasks[0]!.taskId).toBe("tsk-iiii");
  });

  it("sorts tasks by taskId (stable)", async () => {
    const missionId = "msn-7";
    const tasks = [
      makeTask("tsk-zzzz", missionId),
      makeTask("tsk-aaaa", missionId),
      makeTask("tsk-mmmm", missionId),
    ];
    const deps = makeDeps({ tasks });
    const snapshot = await buildAutopilotSnapshot(deps, missionId);

    const ids = snapshot.tasks.map((r) => r.taskId);
    expect(ids).toEqual(["tsk-aaaa", "tsk-mmmm", "tsk-zzzz"]);
  });
});
