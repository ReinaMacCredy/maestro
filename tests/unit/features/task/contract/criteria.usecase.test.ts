import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import {
  addContractCriterion,
  markContractCriterion,
  removeContractCriterion,
} from "@/features/task/usecases/contract/criteria.usecase.js";
import { MaestroError } from "@/shared/errors.js";

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

describe("contract criteria usecases", () => {
  let tmpDir: string;
  let store: FsContractStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "criteria-contract-"));
    store = new FsContractStoreAdapter(tmpDir);
  });

  it("adds, marks, and removes criteria while appending amendment history", async () => {
    const created = await store.create(createInput());
    const locked = await store.save({
      ...created,
      status: "locked",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
    });

    const added = await addContractCriterion(store, {
      ref: locked.id,
      actorId: "session:codex:b",
      text: "receipt hint exists",
    });
    expect(added.status).toBe("amended");
    expect(added.doneWhen).toHaveLength(2);
    const addedCriterion = added.doneWhen[1];
    expect(addedCriterion?.id).toMatch(/^dw-[0-9a-f]{6}$/);

    const marked = await markContractCriterion(store, {
      ref: added.id,
      actorId: "session:codex:b",
      criterionId: addedCriterion!.id,
      evidence: "manual",
    });
    expect(marked.doneWhen[1]).toEqual(
      expect.objectContaining({
        id: addedCriterion!.id,
        met: true,
        metBy: "session:codex:b",
        metEvidence: "manual",
      }),
    );
    expect(marked.amendments).toHaveLength(2);

    const unmarked = await markContractCriterion(store, {
      ref: marked.id,
      actorId: "session:codex:b",
      criterionId: addedCriterion!.id,
      met: false,
    });
    expect(unmarked.doneWhen[1]).toEqual({
      id: addedCriterion!.id,
      text: "receipt hint exists",
      kind: "manual",
    });

    const removed = await removeContractCriterion(store, {
      ref: unmarked.id,
      actorId: "session:codex:b",
      criterionId: addedCriterion!.id,
    });
    expect(removed.doneWhen.map((criterion) => criterion.id)).toEqual(["dw-a1b2c3"]);
    expect(removed.amendments).toHaveLength(4);
  });

  it("rejects unmet evidence and missing criteria", async () => {
    const created = await store.create(createInput());
    const locked = await store.save({
      ...created,
      status: "locked",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
    });

    await expect(
      markContractCriterion(store, {
        ref: locked.id,
        actorId: "session:codex:b",
        criterionId: "dw-a1b2c3",
        met: false,
        evidence: "nope",
      }),
    ).rejects.toThrow(MaestroError);

    await expect(
      removeContractCriterion(store, {
        ref: locked.id,
        actorId: "session:codex:b",
        criterionId: "dw-missing",
      }),
    ).rejects.toThrow(MaestroError);
  });
});
