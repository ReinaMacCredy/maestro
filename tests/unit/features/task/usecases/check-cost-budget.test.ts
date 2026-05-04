import { describe, expect, it } from "bun:test";
import { checkCostBudget } from "@/features/task/usecases/check-cost-budget.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import type { RunState } from "@/features/task/domain/run-state.js";

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
    scope: { filesExpected: [], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 },
    ...overrides,
  };
}

function makeRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    schemaVersion: 1,
    taskId: "tsk-aaaaaa",
    retryCount: 0,
    wallClockElapsedSeconds: 0,
    lastUpdatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("checkCostBudget", () => {
  it("returns not exhausted when state is undefined", () => {
    const result = checkCostBudget(makeContract(), undefined);
    expect(result.exhausted).toBe(false);
    expect(result.reason).toBeUndefined();
  });

  it("returns not exhausted when contract has no costBudget", () => {
    const contract = makeContract({ costBudget: undefined });
    const state = makeRunState({ retryCount: 100, wallClockElapsedSeconds: 99999 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(false);
    expect(result.reason).toBeUndefined();
  });

  it("returns exhausted with max-retries when retryCount >= maxRetries", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 5, wallClockElapsedSeconds: 0 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(true);
    expect(result.reason).toBe("max-retries");
  });

  it("returns exhausted with max-wall-clock when wallClockElapsedSeconds >= maxWallClockSeconds", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 0, wallClockElapsedSeconds: 3601 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(true);
    expect(result.reason).toBe("max-wall-clock");
  });

  it("returns exhausted with max-tokens when tokensUsed >= maxTokens", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 } });
    const state = makeRunState({ retryCount: 0, wallClockElapsedSeconds: 0, tokensUsed: 100_001 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(true);
    expect(result.reason).toBe("max-tokens");
  });

  it("returns max-retries first when both retries and wall-clock are exhausted", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 5, wallClockElapsedSeconds: 9999 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(true);
    expect(result.reason).toBe("max-retries");
  });

  it("does not check tokens when maxTokens is undefined", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 0, wallClockElapsedSeconds: 0, tokensUsed: 999_999 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(false);
  });

  it("does not trigger token check when tokensUsed is undefined but maxTokens is set", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600, maxTokens: 100_000 } });
    const state = makeRunState({ retryCount: 0, wallClockElapsedSeconds: 0, tokensUsed: undefined });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(false);
  });

  it("returns not exhausted when retries and wall-clock are both under budget", () => {
    const contract = makeContract({ costBudget: { maxRetries: 5, maxWallClockSeconds: 3600 } });
    const state = makeRunState({ retryCount: 4, wallClockElapsedSeconds: 3599 });
    const result = checkCostBudget(contract, state);
    expect(result.exhausted).toBe(false);
  });
});
