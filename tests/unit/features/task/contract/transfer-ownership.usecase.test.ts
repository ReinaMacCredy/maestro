import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { transferContractOwnership } from "@/features/task/usecases/contract/transfer-ownership.usecase.js";

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

describe("transferContractOwnership", () => {
  let tmpDir: string;
  let store: FsContractStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "transfer-contract-"));
    store = new FsContractStoreAdapter(tmpDir);
  });

  it("updates lockedBy for active contracts and ignores closed ones", async () => {
    const created = await store.create(createInput());
    const locked = await store.save({
      ...created,
      status: "locked",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
    });

    const transferred = await transferContractOwnership(store, locked.taskId, "session:codex:b");
    expect(transferred?.lockedBy).toBe("session:codex:b");
    expect(transferred?.ownershipHistory).toEqual([
      expect.objectContaining({
        from: "session:codex:a",
        to: "session:codex:b",
        reason: "claim_reclaim",
      }),
    ]);

    const fulfilled = await store.save({
      ...transferred!,
      status: "fulfilled",
      closedAt: "2026-04-21T00:20:00.000Z",
      closedBy: "session:codex:b",
      verdict: {
        fulfilled: true,
        computedAt: "2026-04-21T00:20:00.000Z",
        actualFilesTouched: [],
        expectedFilesMatched: [],
        outOfScopeFiles: [],
        forbiddenTouched: [],
        filesExpectedUnused: [],
        unmetCriteria: [],
        metCriteria: [],
      },
    });

    const closed = await transferContractOwnership(store, fulfilled.taskId, "session:codex:c");
    expect(closed?.lockedBy).toBe("session:codex:b");
    expect(closed?.ownershipHistory).toHaveLength(1);
  });
});
