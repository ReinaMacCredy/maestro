import type { Contract } from "../domain/contract/contract-types.js";
import type { RunState } from "../domain/run-state.js";

export type CostBudgetExhaustionReason = "max-retries" | "max-wall-clock" | "max-tokens";

export interface CostBudgetCheck {
  readonly exhausted: boolean;
  readonly reason?: CostBudgetExhaustionReason;
}

export function checkCostBudget(contract: Contract, state: RunState | undefined): CostBudgetCheck {
  if (state === undefined) return { exhausted: false };
  const budget = contract.costBudget;
  if (!budget) return { exhausted: false };
  if (state.retryCount >= budget.maxRetries) return { exhausted: true, reason: "max-retries" };
  if (state.wallClockElapsedSeconds >= budget.maxWallClockSeconds) return { exhausted: true, reason: "max-wall-clock" };
  if (budget.maxTokens !== undefined && state.tokensUsed !== undefined && state.tokensUsed >= budget.maxTokens) {
    return { exhausted: true, reason: "max-tokens" };
  }
  return { exhausted: false };
}
