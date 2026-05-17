import { describe, expect, it } from "bun:test";
import { requestVerdict } from "@/features/verdict/usecases/request-verdict.usecase.js";
import type { RequestVerdictDeps } from "@/features/verdict/usecases/request-verdict.usecase.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { Contract } from "@/types/contract.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/types/trust.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import type { ContractVersionStorePort, RunStateStorePort, RunStateDelta, GitAnchorPort } from "@/shared/domain/legacy-task";
import type { RunState } from "@/shared/domain/legacy-task/domain/run-state.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import { CONTRACT_SCHEMA_VERSION } from "@/shared/domain/legacy-task/domain/contract/contract-types.js";

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
    findByTreeSha: async (treeSha) => written.filter((v) => v.subject?.tree_sha === treeSha),
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

function fakeGitAnchor(changedPaths: string[] = [], addedLines: string[] = [], treeSha = "deadbeef1234567890abcdef1234567890abcdef"): GitAnchorPort {
  return {
    resolveRepoRoot: async (cwd) => cwd,
    resolveHeadCommit: async () => "abc1234",
    collectTouchedFiles: async () => ({ gitAvailable: true, actualFilesTouched: [] }),
    windowsOverlap: async () => false,
    collectChangedPaths: async () => changedPaths,
    collectAddedLines: async () => addedLines,
    resolveTreeSha: async () => treeSha,
    collectUntrackedFiles: async () => [],
  };
}

function fakeRunStateStore(
  state?: RunState,
): { store: RunStateStorePort; incremented: RunStateDelta[] } {
  const incremented: RunStateDelta[] = [];
  const store: RunStateStorePort = {
    read: async () => state,
    write: async () => {},
    increment: async (_taskId, delta) => {
      incremented.push(delta);
      return {
        schemaVersion: 1,
        taskId: _taskId,
        retryCount: (state?.retryCount ?? 0) + (delta.retryCount ?? 0),
        wallClockElapsedSeconds: (state?.wallClockElapsedSeconds ?? 0) + (delta.wallClockElapsedSeconds ?? 0),
        lastUpdatedAt: new Date().toISOString(),
      };
    },
  };
  return { store, incremented };
}

function makeDeps(overrides: Partial<RequestVerdictDeps> = {}): RequestVerdictDeps {
  const fakeVerdictResult = makeVerdict();
  const { store: verdictStore } = fakeVerdictStore();
  const { store: runStateStore } = fakeRunStateStore();

  return {
    contractVersionStore: fakeContractVersionStore(makeContract()),
    runStateStore,
    evidenceStore: fakeEvidenceStore(),
    verdictStore,
    getEffectiveRiskPolicy: async () => makeRiskPolicy(),
    getEffectiveAutopilotPolicy: async () => makeAutopilotPolicy(),
    getEffectiveReleasePolicy: async () => makeReleasePolicy(),
    getEffectiveSensitivePathsGlobs: async () => [] as readonly string[],
    riskServices: {
      computeRisk: () => fakeVerdictResult,
      deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
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

  it("uses MaestroError with hints when no contract exists (no raw stack trace)", async () => {
    const deps = makeDeps({
      contractVersionStore: fakeContractVersionStore(undefined),
    });
    try {
      await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
      throw new Error("expected requestVerdict to throw");
    } catch (err) {
      const { MaestroError } = await import("@/shared/errors.js");
      expect(err).toBeInstanceOf(MaestroError);
      const hints = (err as { hints?: readonly string[] }).hints ?? [];
      expect(hints.some((h) => h.includes("synthesized automatically"))).toBe(true);
      expect(hints.some((h) => h.includes("maestro task claim tsk-aaaaaa"))).toBe(true);
    }
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
      },
    });
    const result = await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(result.id).toBe(expectedVerdict.id);
    expect(result.decision).toBe("HUMAN");
  });

  it("uses effective policy getters", async () => {
    let usedEffective = false;
    const deps = makeDeps({
      getEffectiveRiskPolicy: async () => {
        usedEffective = true;
        return makeRiskPolicy({ id: "effective-risk" });
      },
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(usedEffective).toBe(true);
  });

  // ─── L4.4: cost-budget and run-state ─────────────────────────────────────────

  it("run-state at retryCount >= maxRetries produces BLOCK verdict", async () => {
    const contract = makeContract({
      costBudget: { maxRetries: 3, maxWallClockSeconds: 3600 },
    });
    const runState: RunState = {
      schemaVersion: 1,
      taskId: "tsk-aaaaaa",
      retryCount: 3,
      wallClockElapsedSeconds: 0,
      lastUpdatedAt: "2026-01-01T00:00:00.000Z",
    };
    const { store: runStateStore } = fakeRunStateStore(runState);

    // Override computeRisk to return BLOCK when costBudgetExhausted is true
    const deps = makeDeps({
      contractVersionStore: fakeContractVersionStore(contract),
      runStateStore,
      riskServices: {
        computeRisk: (input) => {
          if (input.costBudgetExhausted === true) {
            return makeVerdict({ decision: "BLOCK" });
          }
          return makeVerdict({ decision: "PASS" });
        },
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    const result = await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(result.decision).toBe("BLOCK");
  });

  it("FAIL verdict triggers runStateStore.increment with retryCount: 1", async () => {
    const { store: runStateStore, incremented } = fakeRunStateStore();
    const deps = makeDeps({
      runStateStore,
      riskServices: {
        computeRisk: () => makeVerdict({ decision: "FAIL" }),
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(incremented).toHaveLength(1);
    expect(incremented[0]?.retryCount).toBe(1);
  });

  it("HUMAN verdict triggers runStateStore.increment with retryCount: 1", async () => {
    const { store: runStateStore, incremented } = fakeRunStateStore();
    const deps = makeDeps({
      runStateStore,
      riskServices: {
        computeRisk: () => makeVerdict({ decision: "HUMAN" }),
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(incremented).toHaveLength(1);
    expect(incremented[0]?.retryCount).toBe(1);
  });

  it("PASS verdict does NOT trigger runStateStore.increment", async () => {
    const { store: runStateStore, incremented } = fakeRunStateStore();
    const deps = makeDeps({
      runStateStore,
      riskServices: {
        computeRisk: () => makeVerdict({ decision: "PASS" }),
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(incremented).toHaveLength(0);
  });

  it("BLOCK verdict does NOT trigger runStateStore.increment", async () => {
    const { store: runStateStore, incremented } = fakeRunStateStore();
    const deps = makeDeps({
      runStateStore,
      riskServices: {
        computeRisk: () => makeVerdict({ decision: "BLOCK" }),
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(incremented).toHaveLength(0);
  });

  // ─── L5.3: subject stamping ───────────────────────────────────────────────────

  it("stamps subject.tree_sha matching the gitAnchor stub return value", async () => {
    const expectedTreeSha = "1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b";
    const { store: verdictStore, written } = fakeVerdictStore();
    const deps = makeDeps({
      verdictStore,
      gitAnchor: fakeGitAnchor([], [], expectedTreeSha),
    });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(written[0]?.subject?.tree_sha).toBe(expectedTreeSha);
  });

  it("stamps subject.pr when pr is passed in args", async () => {
    const { store: verdictStore, written } = fakeVerdictStore();
    const deps = makeDeps({ verdictStore });
    await requestVerdict({ taskId: "tsk-aaaaaa", pr: 77 }, deps);
    expect(written[0]?.subject?.pr).toBe(77);
  });

  it("omits subject.pr when pr is not passed", async () => {
    const { store: verdictStore, written } = fakeVerdictStore();
    const deps = makeDeps({ verdictStore });
    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect(written[0]?.subject?.pr).toBeUndefined();
    // tree_sha is always present
    expect(typeof written[0]?.subject?.tree_sha).toBe("string");
  });

  it("passes costBudgetExhausted=false to computeRisk when run-state is undefined", async () => {
    let capturedInput: unknown;
    const { store: runStateStore } = fakeRunStateStore(undefined);
    const deps = makeDeps({
      runStateStore,
      riskServices: {
        computeRisk: (input) => {
          capturedInput = input;
          return makeVerdict({ decision: "PASS" });
        },
        deriveRiskClassFromDiff: () => ({ class: "medium", matchedRow: { signal: "diff-source-only" } }),
      },
    });

    await requestVerdict({ taskId: "tsk-aaaaaa" }, deps);
    expect((capturedInput as { costBudgetExhausted?: boolean }).costBudgetExhausted).toBe(false);
  });
});
