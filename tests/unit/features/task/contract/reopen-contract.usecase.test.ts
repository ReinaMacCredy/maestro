import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { reopenContractForTask } from "@/features/task/usecases/contract/reopen-contract.usecase.js";

function createInput(overrides: Partial<Contract> = {}) {
  return {
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    intent: "Keep task work in one place",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "contract exists",
        kind: "manual" as const,
      },
    ],
    createdAt: "2026-04-21T00:00:00.000Z",
    createdBy: "session:codex:a",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail" as const,
      rebaseFallback: "best-effort" as const,
      staleReclaimContractPolicy: "inherit" as const,
    },
    ...overrides,
  };
}

describe("reopenContractForTask", () => {
  let tmpDir: string;
  let store: FsContractStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "reopen-contract-"));
    store = new FsContractStoreAdapter(tmpDir);
  });

  it("reopens amended contracts as amended while preserving amendment history", async () => {
    const created = await store.create(createInput());
    const fulfilled = await store.save({
      ...created,
      status: "fulfilled",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
      amendments: [
        {
          id: "a-a1b2c3",
          at: "2026-04-21T00:20:00.000Z",
          by: "session:codex:a",
          reason: "expanded scope",
          before: {
            intent: "Keep task work in one place",
            scope: created.scope,
            doneWhen: created.doneWhen,
          },
          after: {
            intent: "Keep task work and tests together",
            scope: {
              filesExpected: ["src/features/task/**", "tests/**"],
              filesForbidden: [],
            },
            doneWhen: created.doneWhen,
          },
        },
      ],
      closedAt: "2026-04-21T01:00:00.000Z",
      closedAtCommit: "abc123",
      closedBy: "session:codex:a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-21T01:00:00.000Z",
        actualFilesTouched: ["README.md"],
        expectedFilesMatched: ["README.md"],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
    });

    const reopened = await reopenContractForTask(store, {
      id: fulfilled.taskId,
      contractId: fulfilled.id,
    });

    expect(reopened?.status).toBe("amended");
    expect(reopened?.amendments).toHaveLength(1);
    expect(reopened?.closedAt).toBeUndefined();
    expect(reopened?.closedAtCommit).toBeUndefined();
    expect(reopened?.closedBy).toBeUndefined();
    expect(reopened?.verdict).toBeUndefined();
  });

  it("reopens untouched contracts as locked", async () => {
    const created = await store.create(createInput());
    const fulfilled = await store.save({
      ...created,
      status: "fulfilled",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
      closedAt: "2026-04-21T01:00:00.000Z",
      closedAtCommit: "abc123",
      closedBy: "session:codex:a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-21T01:00:00.000Z",
        actualFilesTouched: ["README.md"],
        expectedFilesMatched: ["README.md"],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
    });

    const reopened = await reopenContractForTask(store, {
      id: fulfilled.taskId,
      contractId: fulfilled.id,
    });

    expect(reopened?.status).toBe("locked");
  });

  it("rejects reopen when another active contract already owns the repo under fail overlap policy", async () => {
    const created = await store.create(createInput());
    const fulfilled = await store.save({
      ...created,
      status: "fulfilled",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
      closedAt: "2026-04-21T01:00:00.000Z",
      closedAtCommit: "abc123",
      closedBy: "session:codex:a",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-21T01:00:00.000Z",
        actualFilesTouched: ["README.md"],
        expectedFilesMatched: ["README.md"],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
    });

    const overlappingDraft = await store.create(createInput({
      taskId: "tsk-b2c3d4",
    }));
    await store.save({
      ...overlappingDraft,
      status: "locked",
      lockedAt: "2026-04-21T02:00:00.000Z",
      lockedBy: "session:codex:b",
    });

    await expect(reopenContractForTask(store, {
      id: fulfilled.taskId,
      contractId: fulfilled.id,
    })).rejects.toThrow("overlaps an active contract in the same repo");
  });
});
