import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { amendContract } from "@/features/task/usecases/contract/amend-contract.usecase.js";
import { MaestroError } from "@/shared/errors.js";

function createInput(overrides: Partial<Contract> = {}) {
  return {
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    intent: "Keep task work in one place",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: ["src/features/mission/**"],
    },
    doneWhen: [
      {
        id: "dw-a1b2c3",
        text: "contract exists",
        kind: "manual" as const,
        met: true,
        metAt: "2026-04-21T01:00:00.000Z",
        metBy: "session:codex:a",
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

describe("amendContract", () => {
  let tmpDir: string;
  let store: FsContractStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "amend-contract-"));
    store = new FsContractStoreAdapter(tmpDir);
  });

  it("records an append-only amendment and preserves matching criterion ids", async () => {
    const created = await store.create(createInput());
    const locked = await store.save({
      ...created,
      status: "locked",
      lockedAt: "2026-04-21T00:10:00.000Z",
      lockedBy: "session:codex:a",
    });

    const amended = await amendContract(store, {
      ref: locked.id,
      actorId: "session:codex:b",
      reason: "expanded coverage",
      intent: "Keep task work and task tests together",
      scope: {
        filesExpected: ["src/features/task/**", "tests/integration/features/task/**"],
        filesForbidden: ["src/features/mission/**"],
      },
      doneWhen: [
        {
          id: locked.doneWhen[0]?.id,
          text: "contract and tests exist",
          kind: "manual",
        },
        {
          text: "criteria can be marked",
          kind: "manual",
        },
      ],
    });

    expect(amended.status).toBe("amended");
    expect(amended.intent).toBe("Keep task work and task tests together");
    expect(amended.scope.filesExpected).toEqual([
      "src/features/task/**",
      "tests/integration/features/task/**",
    ]);
    expect(amended.doneWhen).toHaveLength(2);
    expect(amended.doneWhen[0]).toEqual({
      id: locked.doneWhen[0]?.id,
      text: "contract and tests exist",
      kind: "manual",
    });
    expect(amended.doneWhen[1]?.id).toMatch(/^dw-[0-9a-f]{6}$/);
    expect(amended.amendments).toHaveLength(1);
    expect(amended.amendments[0]).toEqual(
      expect.objectContaining({
        by: "session:codex:b",
        reason: "expanded coverage",
        before: expect.objectContaining({
          intent: "Keep task work in one place",
        }),
        after: expect.objectContaining({
          intent: "Keep task work and task tests together",
        }),
      }),
    );
  });

  it("rejects empty amendment reasons and inactive contracts", async () => {
    const created = await store.create(createInput());

    await expect(
      amendContract(store, {
        ref: created.id,
        actorId: "session:codex:a",
        reason: "   ",
        intent: created.intent,
        scope: created.scope,
        doneWhen: created.doneWhen,
      }),
    ).rejects.toThrow(MaestroError);
  });
});
