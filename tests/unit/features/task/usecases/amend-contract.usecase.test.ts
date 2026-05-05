import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import type { AmendmentBudget, Contract, ContractAmendment } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import { amendContract } from "@/features/task/usecases/amend-contract.usecase.js";
import { proposeContract } from "@/features/task/usecases/propose-contract.usecase.js";
import { MaestroError } from "@/shared/errors.js";
import { mockEvidenceStore } from "../../../../helpers/mocks.js";

function makeAmendment(overrides: Partial<ContractAmendment> = {}): ContractAmendment {
  return {
    id: "a-000001",
    at: "2026-04-21T02:00:00.000Z",
    by: "session:codex:1",
    reason: "scope expansion",
    before: { intent: "old" },
    after: { intent: "new" },
    ...overrides,
  };
}

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-04-21T00:00:00.000Z",
    intent: "old",
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

describe("amendContract", () => {
  let store: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    const tmpDir = await mkdtemp(join(tmpdir(), "amend-contract-v2-"));
    store = new FsContractVersionStoreAdapter(tmpDir);
  });

  it("amend within budget succeeds — writes new version and contract-amendment Evidence row", async () => {
    const budget: AmendmentBudget = {
      maxAmendments: 3,
      maxPathsPerAmendment: 5,
      forbiddenAmendmentPaths: [],
    };
    await proposeContract(store, makeContract({ amendmentBudget: budget }));

    const evidenceStore = mockEvidenceStore();
    await amendContract(store, evidenceStore, {
      taskId: "tsk-a1b2c3",
      amendment: makeAmendment(),
      addedPaths: ["src/new-file.ts"],
      removedPaths: [],
    });

    // new version written
    const hist = await store.history("tsk-a1b2c3");
    expect(hist).toHaveLength(2);
    expect(hist[1]?.amendments).toHaveLength(1);
    expect(hist[1]?.status).toBe("amended");

    // Evidence row written
    const rows = await evidenceStore.list({ task_id: "tsk-a1b2c3" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.kind).toBe("contract-amendment");
    const payload = rows[0]?.payload as { amendmentId: string; addedPaths: string[] };
    expect(payload.amendmentId).toBe("a-000001");
    expect(payload.addedPaths).toEqual(["src/new-file.ts"]);
  });

  it("amend over maxAmendments rejected — throws, writes contract-amendment-blocked with reason budget_exhausted", async () => {
    const budget: AmendmentBudget = {
      maxAmendments: 1,
      maxPathsPerAmendment: 5,
      forbiddenAmendmentPaths: [],
    };
    // Start with a contract that already has 1 amendment (at the limit)
    const existingAmendment = makeAmendment({ id: "a-000000" });
    await proposeContract(store, makeContract({
      amendmentBudget: budget,
      amendments: [existingAmendment],
    }));

    const evidenceStore = mockEvidenceStore();
    await expect(
      amendContract(store, evidenceStore, {
        taskId: "tsk-a1b2c3",
        amendment: makeAmendment({ id: "a-000001" }),
        addedPaths: ["src/extra.ts"],
        removedPaths: [],
      }),
    ).rejects.toThrow(MaestroError);

    const rows = await evidenceStore.list({ task_id: "tsk-a1b2c3" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.kind).toBe("contract-amendment-blocked");
    const payload = rows[0]?.payload as { reason: string };
    expect(payload.reason).toBe("budget_exhausted");
  });

  it("amend over maxPathsPerAmendment rejected — throws, writes contract-amendment-blocked", async () => {
    const budget: AmendmentBudget = {
      maxAmendments: 10,
      maxPathsPerAmendment: 2,
      forbiddenAmendmentPaths: [],
    };
    await proposeContract(store, makeContract({ amendmentBudget: budget }));

    const evidenceStore = mockEvidenceStore();
    await expect(
      amendContract(store, evidenceStore, {
        taskId: "tsk-a1b2c3",
        amendment: makeAmendment(),
        addedPaths: ["a.ts", "b.ts", "c.ts"],
        removedPaths: [],
      }),
    ).rejects.toThrow(MaestroError);

    const rows = await evidenceStore.list({ task_id: "tsk-a1b2c3" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.kind).toBe("contract-amendment-blocked");
    const payload = rows[0]?.payload as { reason: string };
    expect(payload.reason).toBe("budget_exhausted");
  });

  it("amend with path matching forbiddenAmendmentPaths rejected — throws, writes contract-amendment-blocked with reason forbidden_path", async () => {
    const budget: AmendmentBudget = {
      maxAmendments: 10,
      maxPathsPerAmendment: 10,
      forbiddenAmendmentPaths: ["src/infra/**"],
    };
    await proposeContract(store, makeContract({ amendmentBudget: budget }));

    const evidenceStore = mockEvidenceStore();
    await expect(
      amendContract(store, evidenceStore, {
        taskId: "tsk-a1b2c3",
        amendment: makeAmendment(),
        addedPaths: ["src/infra/commands/new.ts"],
        removedPaths: [],
      }),
    ).rejects.toThrow(MaestroError);

    const rows = await evidenceStore.list({ task_id: "tsk-a1b2c3" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.kind).toBe("contract-amendment-blocked");
    const payload = rows[0]?.payload as { reason: string };
    expect(payload.reason).toBe("forbidden_path");
  });

  it("amendment's after.scope is applied to the new version's scope", async () => {
    // Regression: pre-fix, amendContract spread `current` and only updated
    // `amendments[]` and `status`, so the new version still carried the
    // pre-amendment scope. The Trust Verifier scope check (which reads
    // contract.scope.filesExpected) saw the un-amended scope and emitted
    // an out-of-scope error for paths that the amendment explicitly added.
    await proposeContract(store, makeContract({ amendmentBudget: undefined }));

    const amendment: ContractAmendment = makeAmendment({
      after: {
        scope: {
          filesExpected: ["src/**", ".gitignore"],
          filesForbidden: [],
        },
      },
    });

    const evidenceStore = mockEvidenceStore();
    await amendContract(store, evidenceStore, {
      taskId: "tsk-a1b2c3",
      amendment,
      addedPaths: [".gitignore"],
      removedPaths: [],
    });

    const v2 = await store.readCurrent("tsk-a1b2c3");
    expect(v2?.scope.filesExpected).toEqual(["src/**", ".gitignore"]);
    expect(v2?.amendments).toHaveLength(1);
    expect(v2?.status).toBe("amended");
  });

  it("when contract has no amendmentBudget, amendments are unbounded (success)", async () => {
    await proposeContract(store, makeContract({ amendmentBudget: undefined }));

    const evidenceStore = mockEvidenceStore();
    await amendContract(store, evidenceStore, {
      taskId: "tsk-a1b2c3",
      amendment: makeAmendment(),
      addedPaths: ["any/path/at/all.ts", "another/path.ts", "yet/more.ts"],
      removedPaths: [],
    });

    const rows = await evidenceStore.list({ task_id: "tsk-a1b2c3" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.kind).toBe("contract-amendment");
  });
});
