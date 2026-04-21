import { describe, expect, it } from "bun:test";
import { deleteTaskFlow } from "@/features/task/usecases/delete-task-flow.usecase.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import type { Task } from "@/features/task/domain/task-types.js";
import type { ContractStorePort } from "@/features/task/ports/contract-store.port.js";
import type { TaskContinuationHistoryPort } from "@/features/task/ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "@/features/task/ports/task-continuation-store.port.js";
import type { TaskStorePort } from "@/features/task/ports/task-store.port.js";

const CONTRACT_FIXTURE: Contract = {
  schemaVersion: 1,
  id: "c-a1b2c3",
  taskId: "tsk-a1b2c3",
  repoRoot: "/repo",
  status: "locked",
  createdAt: "2026-04-21T00:00:00.000Z",
  lockedAt: "2026-04-21T00:05:00.000Z",
  intent: "Delete the task cleanly.",
  scope: {
    filesExpected: ["src/features/task/**"],
    filesForbidden: [],
  },
  doneWhen: [
    {
      id: "dw-a1b2c3",
      text: "task deleted cleanly",
      kind: "manual",
    },
  ],
  amendments: [],
  createdBy: "session:test",
  lockedBy: "session:test",
  configSnapshot: {
    strict: false,
    overlapPolicy: "fail",
    rebaseFallback: "best-effort",
    staleReclaimContractPolicy: "inherit",
  },
};

function taskFixture(): Task {
  return {
    id: "tsk-a1b2c3",
    title: "delete me",
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    contractId: CONTRACT_FIXTURE.id,
    createdAt: "2026-04-21T00:00:00.000Z",
    updatedAt: "2026-04-21T00:00:00.000Z",
  };
}

function claimedTaskFixture(): Task {
  return {
    ...taskFixture(),
    assignee: "codex-owner",
    claimedAt: "2026-04-21T00:10:00.000Z",
    lastActivityAt: "2026-04-21T00:10:00.000Z",
  };
}

function createTaskStore(state: { current?: Task; deleteCalls: number }): TaskStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    get: async (id) => (state.current?.id === id ? state.current : undefined),
    all: async () => (state.current ? [state.current] : []),
    create: unused,
    createBatch: unused,
    update: unused,
    claim: unused,
    unclaim: unused,
    block: unused,
    unblock: unused,
    releaseOwned: unused,
    reopen: unused,
    delete: async (id) => {
      if (!state.current || state.current.id !== id) {
        throw new Error(`missing task ${id}`);
      }
      state.deleteCalls += 1;
      const deleted = state.current;
      state.current = undefined;
      return deleted;
    },
    heartbeat: unused,
    findBatchReceipt: unused,
    syncMetadata: unused,
  };
}

function createContinuationStore(state: { deletes: number; failOnce?: boolean }): TaskContinuationStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    getActive: unused,
    getCompleted: unused,
    listActive: unused,
    listCompleted: unused,
    upsertActive: unused,
    archiveCompleted: unused,
    reopen: unused,
    delete: async () => {
      state.deletes += 1;
    },
    deleteCompleted: unused,
  };
}

function createHistoryStore(state: { deletes: number; failOnce?: boolean }): TaskContinuationHistoryPort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    append: unused,
    listRecent: unused,
    delete: async () => {
      state.deletes += 1;
      if (state.failOnce) {
        state.failOnce = false;
        throw new Error("history cleanup failed");
      }
    },
  };
}

function createContractStore(state: { deletes: number; byTask?: Contract }): ContractStorePort {
  const unused = async (..._args: unknown[]) => {
    throw new Error("unused");
  };

  return {
    get: async () => undefined,
    getByTaskId: async (taskId) => (state.byTask?.taskId === taskId ? state.byTask : undefined),
    all: async () => (state.byTask ? [state.byTask] : []),
    readIndex: async () => [],
    create: unused,
    save: unused,
    delete: async () => {
      state.deletes += 1;
      state.byTask = undefined;
      return true;
    },
  };
}

describe("deleteTaskFlow", () => {
  it("runs cleanup before removing the task so retries can succeed after partial failure", async () => {
    const taskState = { current: taskFixture(), deleteCalls: 0 };
    const continuationState = { deletes: 0 };
    const historyState = { deletes: 0, failOnce: true };
    const contractState = { deletes: 0, byTask: CONTRACT_FIXTURE };

    await expect(deleteTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore(continuationState),
      continuationHistory: createHistoryStore(historyState),
      contractStore: createContractStore(contractState),
    }, taskState.current!.id)).rejects.toThrow("history cleanup failed");

    expect(taskState.current?.id).toBe("tsk-a1b2c3");
    expect(taskState.deleteCalls).toBe(0);
    expect(contractState.deletes).toBe(0);

    const deleted = await deleteTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore(continuationState),
      continuationHistory: createHistoryStore(historyState),
      contractStore: createContractStore(contractState),
    }, "tsk-a1b2c3");

    expect(deleted.id).toBe("tsk-a1b2c3");
    expect(taskState.current).toBeUndefined();
    expect(taskState.deleteCalls).toBe(1);
    expect(continuationState.deletes).toBe(2);
    expect(historyState.deletes).toBe(2);
    expect(contractState.deletes).toBe(1);
  });

  it("retries orphan cleanup even when the task is already gone", async () => {
    const continuationState = { deletes: 0 };
    const historyState = { deletes: 0 };
    const contractState = { deletes: 0, byTask: CONTRACT_FIXTURE };

    await expect(deleteTaskFlow({
      taskStore: createTaskStore({ current: undefined, deleteCalls: 0 }),
      continuationStore: createContinuationStore(continuationState),
      continuationHistory: createHistoryStore(historyState),
      contractStore: createContractStore(contractState),
    }, "tsk-a1b2c3")).rejects.toThrow("tsk-a1b2c3");

    expect(continuationState.deletes).toBe(1);
    expect(historyState.deletes).toBe(1);
    expect(contractState.deletes).toBe(1);
    expect(contractState.byTask).toBeUndefined();
  });

  it("rejects invalid task ids before any cleanup path runs", async () => {
    const continuationState = { deletes: 0 };
    const historyState = { deletes: 0 };
    const contractState = { deletes: 0, byTask: CONTRACT_FIXTURE };

    await expect(deleteTaskFlow({
      taskStore: createTaskStore({ current: undefined, deleteCalls: 0 }),
      continuationStore: createContinuationStore(continuationState),
      continuationHistory: createHistoryStore(historyState),
      contractStore: createContractStore(contractState),
    }, "../keep")).rejects.toThrow("Task ../keep not found");

    expect(continuationState.deletes).toBe(0);
    expect(historyState.deletes).toBe(0);
    expect(contractState.deletes).toBe(0);
    expect(contractState.byTask).toBe(CONTRACT_FIXTURE);
  });

  it("requires the owner session or force before deleting a claimed task", async () => {
    const taskState = { current: claimedTaskFixture(), deleteCalls: 0 };

    await expect(deleteTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore({ deletes: 0 }),
      continuationHistory: createHistoryStore({ deletes: 0 }),
      contractStore: createContractStore({ deletes: 0, byTask: CONTRACT_FIXTURE }),
    }, taskState.current!.id)).rejects.toThrow("requires the owner session or --force");

    await expect(deleteTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore({ deletes: 0 }),
      continuationHistory: createHistoryStore({ deletes: 0 }),
      contractStore: createContractStore({ deletes: 0, byTask: CONTRACT_FIXTURE }),
    }, taskState.current!.id, { sessionId: "codex-other" })).rejects.toThrow("current session cannot 'delete' it");

    const deleted = await deleteTaskFlow({
      taskStore: createTaskStore(taskState),
      continuationStore: createContinuationStore({ deletes: 0 }),
      continuationHistory: createHistoryStore({ deletes: 0 }),
      contractStore: createContractStore({ deletes: 0, byTask: CONTRACT_FIXTURE }),
    }, taskState.current!.id, { sessionId: "codex-owner" });

    expect(deleted.id).toBe(taskState.current?.id ?? "tsk-a1b2c3");
    expect(taskState.deleteCalls).toBe(1);
  });
});
