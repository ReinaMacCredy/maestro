import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
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

describe("proposeContract", () => {
  let store: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    const tmpDir = await mkdtemp(join(tmpdir(), "propose-contract-"));
    store = new FsContractVersionStoreAdapter(tmpDir);
  });

  it("proposes v1 successfully", async () => {
    const contract = makeContract();
    await proposeContract(store, contract);

    const v1 = await store.readVersion("tsk-a1b2c3", 1);
    expect(v1?.id).toBe("c-a1b2c3");
    expect(v1?.status).toBe("draft");
  });

  it("re-propose for same task throws (v1 already exists)", async () => {
    const contract = makeContract();
    await proposeContract(store, contract);

    await expect(proposeContract(store, makeContract({ id: "c-b2c3d4" }))).rejects.toThrow(MaestroError);
    await expect(proposeContract(store, makeContract({ id: "c-b2c3d4" }))).rejects.toThrow(
      /already has a v1/,
    );
  });
});
