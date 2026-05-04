import { describe, it, expect } from "bun:test";
import { runCiVerify } from "@/features/ci/usecases/run-ci-verify.js";
import type { RunCiVerifyDeps } from "@/features/ci/usecases/run-ci-verify.js";
import type { CiEnv } from "@/features/ci/domain/ci-env.js";
import type { EvidenceStorePort } from "@/features/evidence/ports/storage.js";
import type { Verdict, VerdictDecision } from "@/features/verdict/domain/types.js";
import { generateVerdictId } from "@/features/verdict/domain/verdict-id.js";
import type { RequestVerdictDeps } from "@/features/verdict/index.js";

// ─── Factories ────────────────────────────────────────────────────────────────

function makeVerdict(decision: VerdictDecision = "PASS"): Verdict {
  return {
    schemaVersion: 1,
    id: generateVerdictId(),
    taskId: "tsk-aaaaaa",
    contractVersion: 1,
    computedAt: new Date().toISOString(),
    decision,
    effectiveRiskClass: "medium",
    reasons: [{ category: "policy", code: "all-checks-passed", message: "ok" }],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
  };
}

function fakeEvidenceStore(appended: { kind: string; witnessLevel: string }[] = []): EvidenceStorePort {
  return {
    append: async (row) => {
      appended.push({ kind: row.kind, witnessLevel: row.witness_level });
    },
    read: async () => undefined,
    list: async () => [],
  };
}

function makeCiEnv(overrides: Partial<CiEnv> = {}): CiEnv {
  return {
    provider: "github-actions",
    pr: 42,
    baseRef: "main",
    headSha: "abc1234",
    repository: "owner/repo",
    ...overrides,
  };
}

function makeVerdictDeps(): RequestVerdictDeps {
  // Minimal stub — we mock the request function itself
  return {} as RequestVerdictDeps;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("runCiVerify", () => {
  it("calls requestVerdict with taskId and base resolved from env", async () => {
    const calls: { taskId: string; base?: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ baseRef: "main" }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async (args, _deps) => {
          calls.push({ taskId: args.taskId, base: args.base });
          return expectedVerdict;
        },
      },
      verdictDeps: makeVerdictDeps(),
    };

    const result = await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
    expect(result).toBe(expectedVerdict);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.taskId).toBe("tsk-aaaaaa");
    expect(calls[0]?.base).toBe("main");
  });

  it("--base overrides env.baseRef", async () => {
    const calls: { base?: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ baseRef: "main" }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async (args, _deps) => {
          calls.push({ base: args.base });
          return expectedVerdict;
        },
      },
      verdictDeps: makeVerdictDeps(),
    };

    await runCiVerify({ taskId: "tsk-aaaaaa", base: "origin/develop" }, deps);
    expect(calls[0]?.base).toBe("origin/develop");
  });

  it("passes pr from args to requestVerdict (args.pr takes precedence over env.pr)", async () => {
    const calls: { taskId: string; pr?: number }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ pr: 55 }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async (args, _deps) => {
          calls.push({ taskId: args.taskId, pr: args.pr });
          return expectedVerdict;
        },
      },
      verdictDeps: makeVerdictDeps(),
    };

    await runCiVerify({ taskId: "tsk-aaaaaa", pr: 99 }, deps);
    expect(calls[0]?.taskId).toBe("tsk-aaaaaa");
    // args.pr (99) takes precedence over env.pr (55)
    expect(calls[0]?.pr).toBe(99);
  });

  it("passes pr from env.pr when args.pr is not provided", async () => {
    const calls: { pr?: number }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ pr: 42 }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async (args, _deps) => {
          calls.push({ pr: args.pr });
          return expectedVerdict;
        },
      },
      verdictDeps: makeVerdictDeps(),
    };

    await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
    expect(calls[0]?.pr).toBe(42);
  });

  it("injects CI test results as witnessed-by-ci command evidence when testResultsPath provided", async () => {
    const appended: { kind: string; witnessLevel: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv(),
      evidenceStore: fakeEvidenceStore(appended),
      verdict: {
        request: async () => expectedVerdict,
      },
      verdictDeps: makeVerdictDeps(),
      readTestResults: async (_path) => ({
        passed: 10,
        failed: 0,
        total: 10,
        duration_ms: 1234,
      }),
    };

    await runCiVerify({ taskId: "tsk-aaaaaa", testResultsPath: "/tmp/results.json" }, deps);
    expect(appended).toHaveLength(1);
    expect(appended[0]?.kind).toBe("command");
    expect(appended[0]?.witnessLevel).toBe("witnessed-by-ci");
  });

  it("skips test results ingestion gracefully when testResultsPath is undefined", async () => {
    const appended: { kind: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv(),
      evidenceStore: fakeEvidenceStore(appended as { kind: string; witnessLevel: string }[]),
      verdict: {
        request: async () => expectedVerdict,
      },
      verdictDeps: makeVerdictDeps(),
    };

    await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
    expect(appended).toHaveLength(0);
  });

  it("skips test results ingestion when readTestResults returns undefined", async () => {
    const appended: { kind: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv(),
      evidenceStore: fakeEvidenceStore(appended as { kind: string; witnessLevel: string }[]),
      verdict: {
        request: async () => expectedVerdict,
      },
      verdictDeps: makeVerdictDeps(),
      readTestResults: async () => undefined,
    };

    await runCiVerify({ taskId: "tsk-aaaaaa", testResultsPath: "/tmp/results.json" }, deps);
    expect(appended).toHaveLength(0);
  });

  it("writes 3 GITHUB_OUTPUT keys when outputPath is set", async () => {
    const outputs: { key: string; value: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ outputPath: "/tmp/github-output" }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async () => expectedVerdict,
      },
      verdictDeps: makeVerdictDeps(),
      writeOutput: async (key, value) => {
        outputs.push({ key, value });
      },
    };

    await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
    expect(outputs).toHaveLength(3);
    expect(outputs.find((o) => o.key === "verdict_id")?.value).toBe(expectedVerdict.id);
    expect(outputs.find((o) => o.key === "verdict_decision")?.value).toBe("PASS");
    expect(outputs.find((o) => o.key === "effective_risk_class")?.value).toBe("medium");
  });

  it("does not call writeOutput when outputPath is not set", async () => {
    const outputs: { key: string }[] = [];
    const expectedVerdict = makeVerdict("PASS");

    const deps: RunCiVerifyDeps = {
      env: makeCiEnv({ outputPath: undefined }),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async () => expectedVerdict,
      },
      verdictDeps: makeVerdictDeps(),
      writeOutput: async (key) => {
        outputs.push({ key });
      },
    };

    await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
    expect(outputs).toHaveLength(0);
  });

  it("returns the verdict from requestVerdict unchanged", async () => {
    const verdicts: Verdict[] = [
      makeVerdict("PASS"),
      makeVerdict("FAIL"),
      makeVerdict("HUMAN"),
      makeVerdict("BLOCK"),
    ];

    for (const expected of verdicts) {
      const deps: RunCiVerifyDeps = {
        env: makeCiEnv(),
        evidenceStore: fakeEvidenceStore(),
        verdict: { request: async () => expected },
        verdictDeps: makeVerdictDeps(),
      };
      const result = await runCiVerify({ taskId: "tsk-aaaaaa" }, deps);
      expect(result.decision).toBe(expected.decision);
    }
  });

  it("propagates error from requestVerdict", async () => {
    const deps: RunCiVerifyDeps = {
      env: makeCiEnv(),
      evidenceStore: fakeEvidenceStore(),
      verdict: {
        request: async () => {
          throw new Error("No contract found for task tsk-missing");
        },
      },
      verdictDeps: makeVerdictDeps(),
    };

    await expect(runCiVerify({ taskId: "tsk-missing" }, deps)).rejects.toThrow(
      "No contract found for task tsk-missing",
    );
  });
});
