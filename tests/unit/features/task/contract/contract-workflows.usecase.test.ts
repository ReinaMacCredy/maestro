import { describe, expect, it } from "bun:test";
import {
  buildContractWorkflows,
  type ContractAmendmentCommand,
} from "@/features/task";
import { CONTRACT_SCHEMA_VERSION, type Contract, type ContractConfigSnapshot } from "@/features/task/domain/contract/contract-types.js";
import type { Task } from "@/features/task";
import { MaestroError } from "@/shared/errors.js";
import {
  mockContractStore,
  mockGitAnchor,
  mockTaskStore,
} from "../../../../helpers/mocks.js";

const CONFIG: ContractConfigSnapshot = {
  strict: true,
  overlapPolicy: "annotate",
  rebaseFallback: "best-effort",
  staleReclaimContractPolicy: "inherit",
};

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-000001",
    title: "Do the work",
    type: "task",
    priority: 2,
    status: "in_progress",
    labels: [],
    blocks: [],
    blockedBy: [],
    assignee: "agent-a",
    createdAt: "2026-04-20T00:00:00.000Z",
    updatedAt: "2026-04-20T00:00:00.000Z",
    ...overrides,
  };
}

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-000001",
    repoRoot: ".",
    status: "locked",
    createdAt: "2026-04-20T00:00:00.000Z",
    lockedAt: "2026-04-20T00:01:00.000Z",
    intent: "Change task code",
    scope: {
      filesExpected: ["src/**/*.ts"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-000001",
        text: "unit tests pass",
        kind: "manual",
      },
    ],
    claimedAtCommit: "base",
    amendments: [],
    createdBy: "user",
    lockedBy: "agent-a",
    configSnapshot: CONFIG,
    ...overrides,
  };
}

describe("ContractWorkflows", () => {
  it("draft rolls back the contract when task metadata linking fails", async () => {
    const taskStore = {
      ...mockTaskStore([makeTask({ status: "pending" })]),
      syncMetadata: async () => {
        throw new Error("metadata write failed");
      },
    };
    const baseContractStore = mockContractStore();
    let deletedReason: string | undefined;
    const contractStore = {
      ...baseContractStore,
      create: (input: Parameters<typeof baseContractStore.create>[0]) =>
        baseContractStore.create({ ...input, id: "c-000001" }),
      delete: async (id: string, input: Parameters<typeof baseContractStore.delete>[1]) => {
        deletedReason = input.reason;
        return baseContractStore.delete(id, input);
      },
    };
    const contracts = buildContractWorkflows(contractStore, taskStore, mockGitAnchor());

    await expect(contracts.draft({
      taskId: "tsk-000001",
      repoRoot: "/repo",
      intent: "ship it",
      scope: { filesExpected: ["src/**/*.ts"], filesForbidden: [] },
      doneWhen: [{ text: "tests pass" }],
      createdBy: "agent-a",
      configSnapshot: CONFIG,
    })).rejects.toThrow("metadata write failed");

    expect(deletedReason).toBe("task_link_failed");
    expect(await baseContractStore.get("c-000001")).toBeUndefined();
  });

  it("draft preserves the metadata link error when rollback also fails", async () => {
    const linkError = new Error("metadata write failed");
    const rollbackError = new Error("rollback failed");
    const taskStore = {
      ...mockTaskStore([makeTask({ status: "pending" })]),
      syncMetadata: async () => {
        throw linkError;
      },
    };
    const baseContractStore = mockContractStore();
    const contractStore = {
      ...baseContractStore,
      create: (input: Parameters<typeof baseContractStore.create>[0]) =>
        baseContractStore.create({ ...input, id: "c-000001" }),
      delete: async () => {
        throw rollbackError;
      },
    };
    const contracts = buildContractWorkflows(contractStore, taskStore, mockGitAnchor());

    await expect(contracts.draft({
      taskId: "tsk-000001",
      repoRoot: "/repo",
      intent: "ship it",
      scope: { filesExpected: ["src/**/*.ts"], filesForbidden: [] },
      doneWhen: [{ text: "tests pass" }],
      createdBy: "agent-a",
      configSnapshot: CONFIG,
    })).rejects.toBe(linkError);
    expect((linkError as Error & { rollbackError?: unknown }).rollbackError).toBe(rollbackError);
  });

  it("discard unlinks task metadata on a best-effort basis", async () => {
    const taskStore = {
      ...mockTaskStore([makeTask({ contractId: "c-000001" })]),
      syncMetadata: async () => {
        throw new Error("metadata write failed");
      },
    };
    const contractStore = mockContractStore([makeContract({ status: "draft", lockedAt: undefined, lockedBy: undefined })]);
    const contracts = buildContractWorkflows(contractStore, taskStore, mockGitAnchor());

    const discarded = await contracts.discard("c-000001");

    expect(discarded.status).toBe("discarded");
    expect(discarded.discardedAt).toBeDefined();
  });

  it("routes amendment commands through one workflow method", async () => {
    const contractStore = mockContractStore([makeContract()]);
    const contracts = buildContractWorkflows(contractStore, mockTaskStore([makeTask()]), mockGitAnchor());

    const added = await contracts.amend({
      kind: "addCriterion",
      ref: "c-000001",
      actorId: "agent-a",
      text: "review notes resolved",
    });
    const newCriterion = added.doneWhen.find((criterion) => criterion.text === "review notes resolved");
    expect(newCriterion).toBeDefined();
    expect(added.amendments).toHaveLength(1);

    const marked = await contracts.amend({
      kind: "markCriterion",
      ref: "c-000001",
      actorId: "agent-a",
      criterionId: newCriterion!.id,
      evidence: "review passed",
    });
    expect(marked.doneWhen.find((criterion) => criterion.id === newCriterion!.id)?.met).toBe(true);

    const removed = await contracts.amend({
      kind: "removeCriterion",
      ref: "c-000001",
      actorId: "agent-a",
      criterionId: newCriterion!.id,
    });
    expect(removed.doneWhen.some((criterion) => criterion.id === newCriterion!.id)).toBe(false);

    const replace: ContractAmendmentCommand = {
      kind: "replace",
      ref: "c-000001",
      actorId: "agent-a",
      reason: "scope changed",
      intent: "Change only workflow code",
      scope: { filesExpected: ["src/features/task/**/*.ts"], filesForbidden: ["src/index.ts"] },
      doneWhen: [{ id: "dw-000001", text: "task contract tests pass", kind: "manual" }],
    };
    const replaced = await contracts.amend(replace);
    expect(replaced.intent).toBe("Change only workflow code");
    expect(replaced.scope.filesForbidden).toEqual(["src/index.ts"]);
    expect(replaced.status).toBe("amended");
  });

  it("previewVerdict reports overlap annotations from the captured git anchor", async () => {
    const contract = makeContract({ id: "c-000001", claimedAtCommit: "base-a" });
    // Overlap requires the candidate to have a closed verdict whose
    // actualFilesTouched intersects with ours: open candidates are not
    // counted (they haven't raced on actual files yet).
    const other = makeContract({
      id: "c-000002",
      taskId: "tsk-000002",
      claimedAtCommit: "base-b",
      status: "fulfilled",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-20T00:00:00.000Z",
        actualFilesTouched: ["src/features/task/usecases/contract-workflows.usecase.ts"],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
    });
    const contractStore = mockContractStore([contract, other]);
    const contracts = buildContractWorkflows(
      contractStore,
      mockTaskStore([makeTask()]),
      mockGitAnchor({
        collectTouchedFiles: async () => ({
          gitAvailable: true,
          actualFilesTouched: ["src/features/task/usecases/contract-workflows.usecase.ts"],
          closedAtCommit: "head-a",
        }),
        windowsOverlap: async () => true,
      }),
    );

    const result = await contracts.previewVerdict({
      contract,
      task: makeTask(),
    });

    expect(result.verdict.overlapDetected).toEqual({
      otherContractIds: ["c-000002"],
      policy: "annotate",
    });
  });

  it("closeForTask saves a terminal verdict and preserves ownership notes", async () => {
    const contract = makeContract({
      doneWhen: [{ id: "dw-000001", text: "tests pass", kind: "manual", met: true }],
      ownershipHistory: [
        {
          from: "agent-a",
          to: "agent-b",
          at: "2026-04-20T00:02:00.000Z",
          reason: "handoff_pickup",
        },
      ],
    });
    const task = makeTask({ assignee: "agent-b", contractId: contract.id });
    const contractStore = mockContractStore([contract]);
    const contracts = buildContractWorkflows(
      contractStore,
      mockTaskStore([task]),
      mockGitAnchor({
        collectTouchedFiles: async () => ({
          gitAvailable: true,
          actualFilesTouched: ["src/features/task/commands/task.command.ts"],
          closedAtCommit: "head",
        }),
      }),
    );

    const closed = await contracts.closeForTask(task, "/repo");

    expect(closed?.status).toBe("fulfilled");
    expect(closed?.closedBy).toBe("agent-b");
    expect(closed?.verdict?.notes).toContain("Ownership chain: agent-a -> agent-b");
  });

  it("prepareReopen blocks when fail policy sees another overlapping active contract", async () => {
    const contract = makeContract({
      status: "fulfilled",
      closedAt: "2026-04-20T01:00:00.000Z",
      closedAtCommit: "head-a",
      closedBy: "agent-a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-20T01:00:00.000Z",
        actualFilesTouched: [],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
      configSnapshot: { ...CONFIG, overlapPolicy: "fail" },
    });
    const overlapping = makeContract({ id: "c-000002", taskId: "tsk-000002" });
    const contracts = buildContractWorkflows(
      mockContractStore([contract, overlapping]),
      mockTaskStore([makeTask({ contractId: contract.id })]),
      mockGitAnchor({ windowsOverlap: async () => true }),
    );

    await expect(
      contracts.prepareReopen({ id: "tsk-000001", contractId: "c-000001" }),
    ).rejects.toBeInstanceOf(MaestroError);
  });

  it("prepareReopen allows unrelated active contracts under fail policy", async () => {
    const contract = makeContract({
      status: "fulfilled",
      closedAt: "2026-04-20T01:00:00.000Z",
      closedAtCommit: "head-a",
      closedBy: "agent-a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-20T01:00:00.000Z",
        actualFilesTouched: [],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
      configSnapshot: { ...CONFIG, overlapPolicy: "fail" },
    });
    const unrelated = makeContract({ id: "c-000002", taskId: "tsk-000002" });
    const contracts = buildContractWorkflows(
      mockContractStore([contract, unrelated]),
      mockTaskStore([makeTask({ contractId: contract.id })]),
      mockGitAnchor({ windowsOverlap: async () => false }),
    );

    await expect(
      contracts.prepareReopen({ id: "tsk-000001", contractId: "c-000001" }),
    ).resolves.toMatchObject({ id: "c-000001" });
  });

  it("prepareReopen fails closed when strict overlap checks are indeterminate", async () => {
    const contract = makeContract({
      status: "fulfilled",
      closedAt: "2026-04-20T01:00:00.000Z",
      closedAtCommit: "head-a",
      closedBy: "agent-a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-20T01:00:00.000Z",
        actualFilesTouched: [],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
      configSnapshot: { ...CONFIG, overlapPolicy: "fail" },
    });
    const active = makeContract({ id: "c-000002", taskId: "tsk-000002" });
    let overlapChecks = 0;
    const contracts = buildContractWorkflows(
      mockContractStore([contract, active]),
      mockTaskStore([makeTask({ contractId: contract.id })]),
      mockGitAnchor({
        windowsOverlap: async () => {
          overlapChecks += 1;
          return undefined;
        },
      }),
    );

    await expect(
      contracts.prepareReopen({ id: "tsk-000001", contractId: "c-000001" }),
    ).rejects.toBeInstanceOf(MaestroError);
    expect(overlapChecks).toBe(1);
  });

  it("prepareReopen keeps fail-closed behavior when the closed contract has no commit anchor", async () => {
    const contract = makeContract({
      status: "fulfilled",
      closedAt: "2026-04-20T01:00:00.000Z",
      closedBy: "agent-a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-20T01:00:00.000Z",
        actualFilesTouched: [],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
      configSnapshot: { ...CONFIG, overlapPolicy: "fail" },
    });
    const active = makeContract({ id: "c-000002", taskId: "tsk-000002" });
    let overlapChecks = 0;
    const contracts = buildContractWorkflows(
      mockContractStore([contract, active]),
      mockTaskStore([makeTask({ contractId: contract.id })]),
      mockGitAnchor({
        windowsOverlap: async () => {
          overlapChecks += 1;
          return false;
        },
      }),
    );

    await expect(
      contracts.prepareReopen({ id: "tsk-000001", contractId: "c-000001" }),
    ).rejects.toBeInstanceOf(MaestroError);
    expect(overlapChecks).toBe(0);
  });

  it("transferOwnership updates active owners and ignores terminal contracts", async () => {
    const active = makeContract();
    const terminal = makeContract({ id: "c-000002", taskId: "tsk-000002", status: "fulfilled" });
    const contractStore = mockContractStore([active, terminal]);
    const contracts = buildContractWorkflows(
      contractStore,
      mockTaskStore([makeTask()]),
      mockGitAnchor(),
    );

    const transferred = await contracts.transferOwnership("tsk-000001", "agent-b", "handoff_pickup");
    const ignored = await contracts.transferOwnership("tsk-000002", "agent-b", "handoff_pickup");

    expect(transferred?.lockedBy).toBe("agent-b");
    expect(transferred?.ownershipHistory?.at(-1)).toMatchObject({
      from: "agent-a",
      to: "agent-b",
      reason: "handoff_pickup",
    });
    expect(ignored?.lockedBy).toBe("agent-a");
  });
});
