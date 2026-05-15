import { describe, it, expect } from "bun:test";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { AutopilotPolicy } from "@/features/policy/index.js";
import type { Spec } from "@/shared/domain/legacy-spec/index.js";
import type { Contract } from "@/v2/types/contract.js";
import type { Verdict } from "@/features/verdict/index.js";
import {
  autoMergeEligible,
  type AutoMergeEligibleInput,
} from "@/features/merge/usecases/auto-merge-eligible.usecase.js";

// ---- Fixtures ---------------------------------------------------------------

function makeVerdict(overrides: Partial<Verdict> = {}): Verdict {
  return {
    schemaVersion: 1,
    id: "vrd-test-001",
    taskId: "tsk-test-001",
    contractVersion: 1,
    computedAt: "2026-05-05T00:00:00.000Z",
    decision: "PASS",
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...overrides,
  };
}

function makeAutopilotPolicy(overrides: Partial<AutopilotPolicy> = {}): AutopilotPolicy {
  return {
    id: "autopilot-default",
    kind: "autopilot",
    autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "witnessed-by-ci",
      high: "witnessed-by-ci",
      critical: "witnessed-by-maestro",
    },
    version: "1",
    ...overrides,
  };
}

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 2,
    id: "ctr-test-001",
    taskId: "tsk-test-001",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-05-05T00:00:00.000Z",
    intent: "Test contract",
    scope: {
      filesExpected: ["src/**"],
      filesForbidden: [],
    },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...overrides,
  };
}

function makeEvidence(overrides: Partial<EvidenceRow> = {}): EvidenceRow {
  return {
    schema_version: 3,
    id: "ev-test-001",
    task_id: "tsk-test-001",
    kind: "command",
    witness_level: "witnessed-by-ci",
    created_at: "2026-05-05T00:00:00.000Z",
    payload: { command: "bun test", exit: 0 },
    ...overrides,
  } as EvidenceRow;
}

function makeRollbackEvidence(overrides: Partial<EvidenceRow> = {}): EvidenceRow {
  return {
    schema_version: 3,
    id: "ev-rollback-001",
    task_id: "tsk-test-001",
    kind: "rollback-exercised",
    witness_level: "witnessed-by-ci",
    created_at: "2026-05-05T00:00:00.000Z",
    payload: { command: "bun run rollback", exit: 0 },
    ...overrides,
  } as EvidenceRow;
}

function makeSpec(overrides: Partial<Spec> = {}): Spec {
  return {
    schema_version: 2,
    mission_id: "2026-05-05-001",
    acceptance_criteria: [{ id: "cr-1", text: "Tests pass" }],
    non_goals: [{ text: "No new dependencies" }],
    runtime_signals: [],
    // rollout_plan is what brings L7 into scope, which makes
    // rollback-not-witnessed applicable. Tests that need to assert
    // L7-applies-here behavior rely on this default.
    rollout_plan: {
      feature_flag: "test-flag",
      canary: { stages: [{ percent: 10, hold_minutes: 5 }] },
    },
    created_at: "2026-05-05T00:00:00.000Z",
    updated_at: "2026-05-05T00:00:00.000Z",
    ...overrides,
  };
}

/** Builds a fully-passing input. */
function makePassingInput(): AutoMergeEligibleInput {
  return {
    verdict: makeVerdict(),
    evidenceRows: [makeRollbackEvidence()],
    changedPaths: ["src/features/foo/bar.ts"],
    sensitiveGlobs: [],
    contract: makeContract(),
    autopilotPolicy: makeAutopilotPolicy(),
    spec: makeSpec(),
  };
}

// ---- Tests -------------------------------------------------------------------

describe("autoMergeEligible", () => {
  it("happy path — all clear → eligible:true, reasons:[]", () => {
    const result = autoMergeEligible(makePassingInput());
    expect(result.eligible).toBe(true);
    expect(result.reasons).toEqual([]);
  });

  it("predicate 1 — verdict-not-pass fires when decision !== PASS", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      verdict: makeVerdict({ decision: "FAIL" }),
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("verdict-not-pass");
  });

  it("predicate 2 — auto-merge-class-disabled fires when policy blocks risk class", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      verdict: makeVerdict({ effectiveRiskClass: "high" }),
      // autoMergeAllowed.high = false by default in makeAutopilotPolicy
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("auto-merge-class-disabled");
  });

  it("predicate 3 — evidence-witness-too-weak fires when gating evidence is agent-claimed-locally", () => {
    const weakEvidence = makeEvidence({
      id: "ev-weak",
      kind: "command",
      witness_level: "agent-claimed-locally",
    });
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      evidenceRows: [weakEvidence, makeRollbackEvidence()],
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const reason = result.reasons.find((r) => r.code === "evidence-witness-too-weak");
    expect(reason).toBeDefined();
    expect(reason?.evidenceIds).toContain("ev-weak");
  });

  it("predicate 4 — forbidden-paths-touched fires when changedPath matches filesForbidden", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      changedPaths: [".github/workflows/ci.yml"],
      contract: makeContract({
        scope: {
          filesExpected: ["src/**"],
          filesForbidden: [".github/workflows/**"],
        },
      }),
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("forbidden-paths-touched");
  });

  it("predicate 5 — sensitive-paths-untouched-without-waiver fires when sensitive globs match and no waiver", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      changedPaths: ["src/auth/secret.ts"],
      sensitiveGlobs: ["src/auth/**"],
      // no verdict-override evidence row
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("sensitive-paths-untouched-without-waiver");
  });

  it("predicate 6 — rollback-not-witnessed fires when no rollback-exercised CI evidence", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      // pass only a command evidence, no rollback
      evidenceRows: [makeEvidence()],
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("rollback-not-witnessed");
  });

  it("predicate 6 — does not fire when the spec has no rollout_plan (L7 out of scope)", () => {
    const { rollout_plan: _ignore, ...specWithoutRollout } = makeSpec();
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      spec: specWithoutRollout as Spec,
      evidenceRows: [makeEvidence()],
    };
    const result = autoMergeEligible(input);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).not.toContain("rollback-not-witnessed");
    expect(result.eligible).toBe(true);
  });

  it("predicate 6 — does not fire when rollback evidence exists but exited non-zero", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      // rollback ran but failed: CI witnessed it, but the gate must not pass
      evidenceRows: [
        makeRollbackEvidence({
          payload: { command: "bun run rollback", exit: 1 },
        }),
      ],
    };
    const result = autoMergeEligible(input);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("rollback-not-witnessed");
  });

  it("predicate 7 — review-ack-missing fires for HUMAN verdict at medium risk without review-ack", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      verdict: makeVerdict({ decision: "HUMAN", effectiveRiskClass: "medium" }),
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("review-ack-missing");
  });

  it("predicate 8 — spec-score-below-threshold fires when spec score < 1.0", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      // no non_goals → score 0.5
      spec: makeSpec({ non_goals: [] }),
    };
    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).toContain("spec-score-below-threshold");
  });

  it("eligible when no spec is provided — predicate 8 short-circuits", () => {
    const input: AutoMergeEligibleInput = {
      ...makePassingInput(),
      spec: undefined,
    };
    const result = autoMergeEligible(input);
    // Should be eligible (no other failures in the passing base)
    expect(result.eligible).toBe(true);
    const codes = result.reasons.map((r) => r.code);
    expect(codes).not.toContain("spec-score-below-threshold");
  });

  it("combiner — all predicates firing returns 8 reasons in deterministic order", () => {
    // Craft an input that fails every single predicate:
    // 1. decision !== PASS  → FAIL
    // 2. autoMergeAllowed.high = false
    // 3. weak gating evidence
    // 4. forbidden path touched
    // 5. sensitive path touched, no waiver
    // 6. no rollback CI evidence
    // 7. HUMAN verdict at medium+ risk, no review-ack
    // 8. spec score < 1.0
    const weakCmdEvidence = makeEvidence({
      id: "ev-weak-cmd",
      kind: "command",
      witness_level: "agent-claimed-locally",
    });
    const input: AutoMergeEligibleInput = {
      verdict: makeVerdict({ decision: "HUMAN", effectiveRiskClass: "high" }),
      evidenceRows: [weakCmdEvidence],
      changedPaths: [".github/workflows/ci.yml", "src/auth/secret.ts"],
      sensitiveGlobs: ["src/auth/**"],
      contract: makeContract({
        scope: {
          filesExpected: ["src/**"],
          filesForbidden: [".github/workflows/**"],
        },
      }),
      autopilotPolicy: makeAutopilotPolicy(),
      spec: makeSpec({ non_goals: [] }),
    };

    const result = autoMergeEligible(input);
    expect(result.eligible).toBe(false);

    const codes = result.reasons.map((r) => r.code);
    expect(codes).toHaveLength(8);

    // Must match the exact order of the union definition
    expect(codes).toEqual([
      "verdict-not-pass",
      "auto-merge-class-disabled",
      "evidence-witness-too-weak",
      "forbidden-paths-touched",
      "sensitive-paths-untouched-without-waiver",
      "rollback-not-witnessed",
      "review-ack-missing",
      "spec-score-below-threshold",
    ]);
  });
});
