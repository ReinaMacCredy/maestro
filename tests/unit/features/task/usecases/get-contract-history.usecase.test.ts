import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import { getContractHistory } from "@/features/task/usecases/get-contract-history.usecase.js";
import { proposeContract } from "@/features/task/usecases/propose-contract.usecase.js";
import { approveContract } from "@/features/task/usecases/approve-contract.usecase.js";

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

describe("getContractHistory", () => {
  let store: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    const tmpDir = await mkdtemp(join(tmpdir(), "get-contract-history-"));
    store = new FsContractVersionStoreAdapter(tmpDir);
  });

  it("returns versions in ascending order after propose + approve", async () => {
    await proposeContract(store, makeContract());
    await approveContract(store, "tsk-a1b2c3", "session:codex:1", "2026-04-21T01:00:00.000Z");

    const hist = await getContractHistory(store, "tsk-a1b2c3");
    expect(hist).toHaveLength(2);
    expect(hist[0]?.status).toBe("draft");
    expect(hist[1]?.status).toBe("locked");
  });

  it("returns empty list when no contract has been proposed", async () => {
    const hist = await getContractHistory(store, "tsk-a1b2c3");
    expect(hist).toHaveLength(0);
  });
});
