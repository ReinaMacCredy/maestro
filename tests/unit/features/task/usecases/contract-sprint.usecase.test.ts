import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter } from "@/features/evidence";
import { contractSprint } from "@/features/task/usecases/contract-sprint.usecase.js";
import type {
  ContractStorePort,
} from "@/features/task/ports/contract-store.port.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";

class FakeVersionStore implements ContractVersionStorePort {
  constructor(private contracts: Contract[] = []) {}
  push(c: Contract): void {
    this.contracts.push(c);
  }
  async write(): Promise<void> {
    /* noop */
  }
  async readCurrent(taskId: string): Promise<Contract | undefined> {
    const list = this.contracts.filter((c) => c.taskId === taskId);
    return list.at(-1);
  }
  async readVersion(): Promise<Contract | undefined> {
    return undefined;
  }
  async history(taskId: string): Promise<readonly Contract[]> {
    return this.contracts.filter((c) => c.taskId === taskId);
  }
}

const fakeStore: ContractStorePort = {
  async getByTaskId() {
    return undefined;
  },
} as unknown as ContractStorePort;

function activeContract(taskId: string, overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 1,
    id: "ctr-1",
    taskId,
    repoRoot: "/r",
    status: "locked",
    createdAt: "2026-01-01T00:00:00Z",
    intent: "test",
    scope: { filesExpected: [], filesForbidden: [] },
    doneWhen: [
      { id: "d-1", text: "criteria one", kind: "manual", met: true },
      { id: "d-2", text: "criteria two", kind: "manual" },
    ],
    amendments: [],
    createdBy: { kind: "agent", id: "test" } as unknown as Contract["createdBy"],
    configSnapshot: { strict: false, overlapPolicy: "annotate", rebaseFallback: "best-effort", staleReclaimContractPolicy: "inherit" } as Contract["configSnapshot"],
    amendmentBudget: { maxAmendments: 3, maxPathsPerAmendment: 5, forbiddenAmendmentPaths: [] } as Contract["amendmentBudget"],
    ...overrides,
  } as Contract;
}

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "sprint-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe("contractSprint", () => {
  it("returns snapshot with met/total criteria when contract exists", async () => {
    const versionStore = new FakeVersionStore([activeContract("tsk-aaa111")]);
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    const r = await contractSprint(
      { contractVersionStore: versionStore, contractStore: fakeStore, evidenceStore },
      { taskId: "tsk-aaa111" },
    );
    expect(r.snapshot.criteriaCount).toBe(2);
    expect(r.snapshot.metCount).toBe(1);
    expect(r.snapshot.amendmentBudget?.maxAmendments).toBe(3);
    expect(r.snapshot.amendmentBudget?.remaining).toBe(3);
    expect(r.proposalRecorded).toBeUndefined();
  });

  it("records a proposal as evidence when --propose is supplied", async () => {
    const versionStore = new FakeVersionStore([activeContract("tsk-aaa111")]);
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    const r = await contractSprint(
      { contractVersionStore: versionStore, contractStore: fakeStore, evidenceStore },
      { taskId: "tsk-aaa111", propose: "add new acceptance criterion: integration tests", proposedBy: "agent-x" },
    );
    expect(r.proposalRecorded?.evidenceId).toBeTruthy();
    expect(r.proposalRecorded?.proposal).toContain("integration tests");
    const list = await evidenceStore.list({ task_id: "tsk-aaa111" });
    expect(list.length).toBe(1);
    expect(list[0]!.kind).toBe("manual-note");
  });

  it("ignores empty --propose strings", async () => {
    const versionStore = new FakeVersionStore([activeContract("tsk-aaa111")]);
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    const r = await contractSprint(
      { contractVersionStore: versionStore, contractStore: fakeStore, evidenceStore },
      { taskId: "tsk-aaa111", propose: "   " },
    );
    expect(r.proposalRecorded).toBeUndefined();
    const list = await evidenceStore.list({ task_id: "tsk-aaa111" });
    expect(list.length).toBe(0);
  });

  it("returns snapshot with no contract when none exists", async () => {
    const versionStore = new FakeVersionStore();
    const evidenceStore = new FsEvidenceStoreAdapter(dir);
    const r = await contractSprint(
      { contractVersionStore: versionStore, contractStore: fakeStore, evidenceStore },
      { taskId: "tsk-zzz999" },
    );
    expect(r.snapshot.contractId).toBeUndefined();
    expect(r.snapshot.criteriaCount).toBe(0);
  });
});
