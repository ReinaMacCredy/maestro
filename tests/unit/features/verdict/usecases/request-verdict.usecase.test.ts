import { describe, expect, it } from "bun:test";
import { requestVerdict } from "@/features/verdict/usecases/request-verdict.usecase.js";
import type { RequestVerdictDeps } from "@/features/verdict/usecases/request-verdict.usecase.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { Contract } from "@/features/task/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/features/verify/index.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { ContractVersionStorePort } from "@/features/task/ports/contract-version-store.port.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { GitAnchorPort } from "@/features/task/ports/git-anchor.port.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

// ─── Factories ─────────────────────────────────────────────────────────────────

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test task",
    scope: { filesExpected: ["src/foo.ts"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
    amendmentBudget: { maxAmendments: 4, maxPathsPerAmendment: 5, forbiddenAmendmentPaths: [] },
    ...overrides,
  };
}

function makeEvidenceRow(overrides: Partial<EvidenceRow<"command">> = {}): EvidenceRow<"command"> {
  return {
    schema_version: 3,
    id: `evd-${"a".repeat(13)}-aaaaaa`,
    task_id: "tsk-aaaaaa",
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: { command: "bun test", exit: 0 },
    ...overrides,
  };
}

function makeRiskPolicy(overrides: Partial<RiskPolicy> = {}): RiskPolicy {
  return {
    kind: "risk",
    id: "risk-policy-test",
    version: "1",
    rows: [],
    ...overrides,
  };
}

function makeAutopilotPolicy(overrides: Partial<AutopilotPolicy> = {}): AutopilotPolicy {
  return {
    kind: "autopilot",
    id: "autopilot-policy-test",
    version: "1",
    autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "agent-claimed-locally",
      high: "witnessed-by-maestro",
      critical: "witnessed-by-maestro",
    },
    ...overrides,
  };
}

function makeReleasePolicy(overrides: Partial<ReleasePolicy> = {}): ReleasePolicy {
  return {
    kind: "release",
    id: "release-policy-test",
    version: "1",
    requireSignedCommits: false,
    requireProofMapComplete: false,
    ...overrides,
  };
}

function makeVerdict(overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: "2026-05-04T10:00:00.000Z",
    decision: "PASS",
    effectiveRiskClass: "medium",
    proposedRiskClass: "medium",
    reasons: [{ category: "policy", code: "all-checks-passed", message: "All checks passed." }],
    evidenceConsulted: [],
    policiesConsulted: [
      { file: "policies/risk.yaml", version: "1" },
      { file: "policies/autopilot.yaml", version: "1" },
      { file: "policies/release.yaml", version: "1" },
    ],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...overrides,
  };
}

// ─── Mock helpers ─────────────────────────────────────────────────────────────

function fakeContractVersionStore(contract?: Contract): ContractVersionStorePort {
  return {
    write: async () => {},
    readCurrent: async () => contract,
    readVersion: async () => contract,
    history: async () => (contract !== undefined ? [contract] : []),
  };
}

function fakeVerdictStore(): { store: VerdictStorePort; written: Verdict[] } {
  const written: Verdict[] = [];
  const store: VerdictStorePort = {
    write: async (_taskId, verdict) => { written.push(verdict); },
    readLatest: async () => written[written.length - 1],
    readVersion: async (_taskId, id) => written.find((v) => v.id === id),
    history: async () => [...written],
  };
  return { store, written };
}

function fakeEvidenceStore(rows: EvidenceRow[] = []): EvidenceStorePort {
  return {
    append: async () => {},
    read: async () => undefined,
    list: async () => rows,
  };
}

function fakeGitAnchor(changedPaths: string[] = [], addedLines: string[] = []): GitAnchorPort {
  return {
    resolveRepoRoot: async (cwd) => cwd,
    resolveHeadCommit: async () => "abc1234",
    collectTouchedFiles: async () => ({ gitAvailable: true, actualFilesTouched: [] }),
    windowsOverlap: async () => false,
    collectChangedPaths: async () => changedPaths,
    collectAddedLines: async () => addedLines,
  };
}

function makeDeps(overrides: Partial<RequestVerdictDeps> = {}): RequestVerdictDeps {
  const fakeVerdictResult = makeVerdict();
  const { store: verdictStore, written } = fakeVerdictStore();

  return {
    contractVersionStore: fakeContractVersionStore(makeContract()),
    evidenceStore: fakeEvidenceStore(),
    verdictStore,
    getRiskPolicy: async () => makeRiskPolicy(),
    getAutopilotPolicy: async () => makeAutopilotPolicy(),
    getReleasePolicy: async () => makeReleasePolicy(),
    riskServices: {
      computeRisk: () => fakeVerdictResult,
      deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      getEffectivePolicies: async () => ({
        riskPolicy: makeRiskPolicy(),
        autopilotPolicy: makeAutopilotPolicy(),
        releasePolicy: makeReleasePolicy(),
      }),
    },
    runTrustVerifier: async () => ({ findings: [] }),
    gitAnchor: fakeGitAnchor(),
    projectRoot: "/tmp/test-project",
    ...overrides,
  };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("requestVerdict", () => {
  it("throws if no contract exists for the task", async () => {
    const deps = makeDeps({
      contractVersionStore: fakeContractVersionStore(undefined),
    });
    await expect(requestVerdict({ taskId: "tsk-aaaaaa" }, deps)).rejects.toThrow(/No contract found/);
  });

  it("loads the contract from contractVersionStore", async () => {
    let loadedTaskId: string | undefined;
    const contract = makeContract();
    const contractStore: ContractVersionStorePort = {
      write: async () => {},
      readCurrent: async (taskId) => {
        loadedTaskId = taskId;
        return contract;
      },
      readVersion: async () => contract,
      history: async () => [contract],
    };
    const deps = makeDeps({ contractVersionStore: contractStore });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(loadedTaskId).toBe("tsk-aaaaaa");
  });

  it("calls trust verifier with contract and diff", async () => {
    let verifierInput: unknown;
    const deps = makeDeps({
      runTrustVerifier: async (input) => {
        verifierInput = input;
        return { findings: [] };
      },
      gitAnchor: fakeGitAnchor(["src/foo.ts"], ["const x = 1;"]),
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(verifierInput).toBeDefined();
    const input = verifierInput as { diff: { changedPaths: string[] } };
    expect(input.diff.changedPaths).toContain("src/foo.ts");
  });

  it("lists evidence for the task", async () => {
    let listedFilter: unknown;
    const deps = makeDeps({
      evidenceStore: {
        append: async () => {},
        read: async () => undefined,
        list: async (filter) => {
          listedFilter = filter;
          return [];
        },
      },
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect((listedFilter as { task_id: string }).task_id).toBe("tsk-aaaaaa");
  });

  it("calls computeRisk with all collected inputs and returns the result", async () => {
    const evidence = makeEvidenceRow();
    const trustFindings: TrustFinding[] = [{ check: "scope", severity: "info", paths: [] }];
    let computeRiskInput: unknown;
    const expectedVerdict = makeVerdict({ decision: "FAIL" });

    const deps = makeDeps({
      evidenceStore: fakeEvidenceStore([evidence]),
      runTrustVerifier: async () => ({ findings: trustFindings }),
      riskServices: {
        computeRisk: (input) => {
          computeRiskInput = input;
          return expectedVerdict;
        },
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
        getEffectivePolicies: async () => ({
          riskPolicy: makeRiskPolicy(),
          autopilotPolicy: makeAutopilotPolicy(),
          releasePolicy: makeReleasePolicy(),
        }),
      },
    });

    const result = await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(result.decision).toBe("FAIL");
    expect(computeRiskInput).toBeDefined();
    const input = computeRiskInput as { trustFindings: TrustFinding[]; evidenceRows: EvidenceRow[] };
    expect(input.trustFindings).toHaveLength(1);
    expect(input.evidenceRows).toHaveLength(1);
  });

  it("persists the verdict via verdictStore.write", async () => {
    const { store: verdictStore, written } = fakeVerdictStore();
    const deps = makeDeps({ verdictStore });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(written).toHaveLength(1);
    expect(written[0]?.taskId).toBe("tsk-aaaaaa");
  });

  it("returns the same verdict that computeRisk produced", async () => {
    const expectedVerdict = makeVerdict({ decision: "HUMAN" });
    const deps = makeDeps({
      riskServices: {
        computeRisk: () => expectedVerdict,
        deriveRiskClassFromDiff: () => ({ class: "high", matchedRow: { signal: "diff-modifies-ci-workflows" } }),
        getEffectivePolicies: async () => ({
          riskPolicy: makeRiskPolicy(),
          autopilotPolicy: makeAutopilotPolicy(),
          releasePolicy: makeReleasePolicy(),
        }),
      },
    });
    const result = await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(result.id).toBe(expectedVerdict.id);
    expect(result.decision).toBe("HUMAN");
  });

  it("prefers effective policy getters when available", async () => {
    let usedEffective = false;
    const deps = makeDeps({
      getEffectiveRiskPolicy: async () => {
        usedEffective = true;
        return makeRiskPolicy({ id: "effective-risk" });
      },
      getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
      getEffectiveReleasePolicy: async () => makeReleasePolicy(),
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(usedEffective).toBe(true);
  });

  it("falls back to raw policy getters when effective getters are not provided", async () => {
    let usedRaw = false;
    const deps = makeDeps({
      getEffectiveRiskPolicy: undefined,
      getEffectiveAutopilotPolicy: undefined,
      getEffectiveReleasePolicy: undefined,
      getRiskPolicy: async () => {
        usedRaw = true;
        return makeRiskPolicy();
      },
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(usedRaw).toBe(true);
  });
});
