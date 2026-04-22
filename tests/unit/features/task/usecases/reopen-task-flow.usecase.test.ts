import { describe, expect, it } from "bun:test";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import type { Task } from "@/features/task/domain/task-types.js";
import { reopenTaskFlow } from "@/features/task/usecases/reopen-task-flow.usecase.js";
import type { ContractStorePort } from "@/features/task/ports/contract-store.port.js";
import type { TaskContinuationHistoryPort } from "@/features/task/ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "@/features/task/ports/task-continuation-store.port.js";
import type { TaskStorePort } from "@/features/task/ports/task-store.port.js";

function completedTaskFixture(): Task {
  return {
    id: "tsk-a1b2c3",
    title: "reopen me",
    type: "task",
    priority: 2,
    status: "completed",
    labels: [],
    blocks: [],
    blockedBy: [],
    contractId: "c-a1b2c3",
    closeReason: "done",
    createdAt: "2026-04-21T00:00:00.000Z",
    updatedAt: "2026-04-21T00:10:00.000Z",
  };
}

function fulfilledContractFixture(): Contract {
  return {
    schemaVersion: 1,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: ".",
    status: "fulfilled",
    createdAt: "2026-04-21T00:00:00.000Z",
    lockedAt: "2026-04-21T00:05:00.000Z",
    closedAt: "2026-04-21T00:10:00.000Z",
    closedAtCommit: "abc123",
    intent: "Keep reopen state aligned",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "done",
        kind: "manual",
      },
    ],
    amendments: [],
    verdict: {
      fulfilled: true,
      computedAt: "2026-04-21T00:10:00.000Z",
      actualFilesTouched: ["README.md"],
      expectedFilesMatched: ["README.md"],
      outOfScopeFiles: [],
      forbiddenTouched: [],
      filesExpectedUnused: [],
      unmetCriteria: [],
      metCriteria: [],
    },
    createdBy: "session:test",
    lockedBy: "session:test",
    closedBy: "session:test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
  };
}

function createTaskStore(state: { current: Task }): TaskStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    get: async (id) => (state.current.id === id ? state.current : undefined),
    all: async () => [state.current],
    create: unused,
    createBatch: unused,
    update: unused,
    claim: unused,
    unclaim: unused,
    block: unused,
    unblock: unused,
    releaseOwned: unused,
    reopen: async (id) => {
      if (state.current.id !== id) {
        throw new Error(`missing task ${id}`);
      }
      state.current = {
        ...state.current,
        status: "pending",
        closeReason: undefined,
        updatedAt: "2026-04-21T00:20:00.000Z",
      };
      return state.current;
    },
    delete: unused,
    heartbeat: unused,
    findBatchReceipt: unused,
    syncMetadata: unused,
  };
}

function createContinuationStore(state: { active?: unknown; reopened: number }): TaskContinuationStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    getActive: async () => undefined,
    getCompleted: async () => undefined,
    listActive: unused,
    listCompleted: unused,
    upsertActive: async (summary) => {
      state.active = summary;
      return summary;
    },
    archiveCompleted: unused,
    reopen: async () => {
      state.reopened += 1;
      return false;
    },
    delete: unused,
    deleteCompleted: unused,
  };
}

function createHistoryStore(state: { appended: number }): TaskContinuationHistoryPort {
  return {
    append: async () => {
      state.appended += 1;
    },
    listRecent: async () => [],
    delete: async () => {},
  };
}

describe("reopenTaskFlow", () => {
  it("rolls task reopen back when contract reactivation fails", async () => {
    const taskState = { current: completedTaskFixture() };
    const continuationState = { active: undefined as unknown, reopened: 0 };
    const historyState = { appended: 0 };
    const contract = fulfilledContractFixture();

    const contractStore: ContractStorePort = {
      get: async (id) => (id === contract.id ? contract : undefined),
      getByTaskId: async (taskId) => (taskId === contract.taskId ? contract : undefined),
      all: async () => [contract],
      readIndex: async () => [],
      create: async () => {
        throw new Error("unused");
      },
      save: async () => {
        throw new Error("save failed");
      },
      delete: async () => false,
    };

    await expect(reopenTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore(continuationState),
      continuationHistory: createHistoryStore(historyState),
      contractStore,
    }, taskState.current.id)).rejects.toThrow("save failed");

    expect(taskState.current.status).toBe("completed");
    expect(taskState.current.closeReason).toBe("done");
  });
});
