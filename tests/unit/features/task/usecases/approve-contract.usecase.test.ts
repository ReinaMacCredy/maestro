import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import { approveContract } from "@/features/task/usecases/approve-contract.usecase.js";
import { proposeContract } from "@/features/task/usecases/propose-contract.usecase.js";
import { MaestroError } from "@/shared/errors.js";

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    status: "draft",
    createdAt: "2026-04-21T00:00:00.000Z",
    intent: "Implement versioned contracts",
    scope: { filesExpected: ["src/**"], filesForbidden: [] },
    doneWhen: [{ id: "dw-a1b2c3", text: "done", kind: "manual" as const }],
    amendments: [],
    createdBy: "session:codex:1",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail" as const,
      rebaseFallback: "best-effort" as const,
      staleReclaimContractPolicy: "inherit" as const,
    },
    ...overrides,
  };
}

describe("approveContract", () => {
  let store: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    const tmpDir = await mkdtemp(join(tmpdir(), "approve-contract-"));
    store = new FsContractVersionStoreAdapter(tmpDir);
  });

  it("approves a draft contract — writes a new version with status locked", async () => {
    await proposeContract(store, makeContract());
    await approveContract(store, "tsk-a1b2c3", "session:codex:1", "2026-04-21T01:00:00.000Z");

    const current = await store.readCurrent("tsk-a1b2c3");
    expect(current?.status).toBe("locked");
    expect(current?.lockedBy).toBe("session:codex:1");
    expect(current?.lockedAt).toBe("2026-04-21T01:00:00.000Z");

    // v1 is still readable as the draft
    const v1 = await store.readVersion("tsk-a1b2c3", 1);
    expect(v1?.status).toBe("draft");

    // v2 is the locked version
    const v2 = await store.readVersion("tsk-a1b2c3", 2);
    expect(v2?.status).toBe("locked");
  });

  it("re-approving an already-locked contract throws", async () => {
    await proposeContract(store, makeContract());
    await approveContract(store, "tsk-a1b2c3", "session:codex:1", "2026-04-21T01:00:00.000Z");

    await expect(
      approveContract(store, "tsk-a1b2c3", "session:codex:2", "2026-04-21T02:00:00.000Z"),
    ).rejects.toThrow(MaestroError);
    await expect(
      approveContract(store, "tsk-a1b2c3", "session:codex:2", "2026-04-21T02:00:00.000Z"),
    ).rejects.toThrow(/already locked/);
  });

  it("throws when no contract exists for the task", async () => {
    await expect(
      approveContract(store, "tsk-a1b2c3", "session:codex:1", "2026-04-21T01:00:00.000Z"),
    ).rejects.toThrow(MaestroError);
    await expect(
      approveContract(store, "tsk-a1b2c3", "session:codex:1", "2026-04-21T01:00:00.000Z"),
    ).rejects.toThrow(/No contract found/);
  });
});
