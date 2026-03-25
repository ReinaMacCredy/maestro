/**
 * DCP Component Registry -- priority-ordered context components.
 *
 * Defines 10 components that make up a worker's context injection.
 * Priority 0 = highest. Components with priority 0-2 are "protected"
 * and never pruned. Budget overflow removes from the bottom.
 */

import { estimateTokens } from '../../core/tokens.ts';

export interface DcpComponent {
  /** Component identifier. */
  name: string;
  /** Lower number = higher priority. */
  priority: number;
  /** Protected components are never pruned. */
  protected: boolean;
  /** Estimate tokens from assembled content. Returns 0 if content is empty. */
  estimateTokens(content: string): number;
}

export interface PrunedResult {
  /** Components included in the injection, ordered by priority. */
  included: Array<{ name: string; tokens: number }>;
  /** Components dropped due to budget overflow. */
  dropped: Array<{ name: string; tokens: number }>;
  /** Total tokens used. */
  totalTokens: number;
}

function makeComponent(name: string, priority: number): DcpComponent {
  return {
    name,
    priority,
    protected: priority <= 2,
    estimateTokens: (content: string) => estimateTokens(content),
  };
}

/**
 * 10 DCP components sorted by priority (ascending).
 * Priority 0-2: always included (spec, worker-rules, revision).
 * Priority 3-9: prunable when over budget.
 */
export const COMPONENT_REGISTRY: readonly DcpComponent[] = [
  makeComponent('spec', 0),
  makeComponent('worker-rules', 1),
  makeComponent('revision', 2),
  makeComponent('graph', 3),
  makeComponent('completed-tasks', 4),
  makeComponent('doctrine', 5),
  makeComponent('memories', 6),
  makeComponent('skills', 7),
  makeComponent('agent-tools', 8),
  makeComponent('handoff', 9),
] as const;

/**
 * Allocate tokens across components using priority ordering.
 *
 * Protected components always get their full estimate.
 * Remaining budget is distributed top-down among unprotected components.
 *
 * @param totalTokens - Total token budget for the injection.
 * @param assembled - Map of component name to assembled content string.
 * @returns Map of component name to allocated token budget.
 */
export function allocateBudget(
  totalTokens: number,
  assembled: Map<string, string>,
): Map<string, number> {
  const allocations = new Map<string, number>();

  // Phase 1: Protected components always get their full estimate
  let protectedCost = 0;
  for (const comp of COMPONENT_REGISTRY) {
    const content = assembled.get(comp.name) ?? '';
    const tokens = comp.estimateTokens(content);
    if (comp.protected) {
      allocations.set(comp.name, tokens);
      protectedCost += tokens;
    }
  }

  // Phase 2: Distribute remaining budget to unprotected, top-down by priority
  let remaining = Math.max(0, totalTokens - protectedCost);
  for (const comp of COMPONENT_REGISTRY) {
    if (comp.protected) continue;
    const content = assembled.get(comp.name) ?? '';
    const tokens = comp.estimateTokens(content);
    const allocated = Math.min(tokens, remaining);
    allocations.set(comp.name, allocated);
    remaining -= allocated;
  }

  return allocations;
}

/**
 * Prune assembled components to fit within a total token budget.
 *
 * Protected components (priority 0-2) are always included.
 * Unprotected components are dropped from lowest priority (highest number)
 * until the total fits within budget.
 *
 * @param assembled - Map of component name to assembled content string.
 * @param totalBudget - Maximum total tokens for the injection.
 * @returns PrunedResult with included/dropped components.
 */
export function pruneComponents(
  assembled: Map<string, string>,
  totalBudget: number,
): PrunedResult {
  // Build entries with token estimates
  const entries: Array<{ name: string; tokens: number; protected: boolean; priority: number }> = [];
  for (const comp of COMPONENT_REGISTRY) {
    const content = assembled.get(comp.name) ?? '';
    const tokens = comp.estimateTokens(content);
    if (tokens > 0) {
      entries.push({ name: comp.name, tokens, protected: comp.protected, priority: comp.priority });
    }
  }

  // Protected components are always included
  const protectedEntries = entries.filter(e => e.protected);
  const unprotected = entries.filter(e => !e.protected);

  // Sort unprotected by priority ascending (lowest number = most important)
  unprotected.sort((a, b) => a.priority - b.priority);

  const protectedTotal = protectedEntries.reduce((sum, e) => sum + e.tokens, 0);
  let remaining = Math.max(0, totalBudget - protectedTotal);

  const included: Array<{ name: string; tokens: number }> = [
    ...protectedEntries.map(e => ({ name: e.name, tokens: e.tokens })),
  ];
  const dropped: Array<{ name: string; tokens: number }> = [];

  for (const entry of unprotected) {
    if (entry.tokens <= remaining) {
      included.push({ name: entry.name, tokens: entry.tokens });
      remaining -= entry.tokens;
    } else {
      dropped.push({ name: entry.name, tokens: entry.tokens });
    }
  }

  return {
    included,
    dropped,
    totalTokens: included.reduce((sum, e) => sum + e.tokens, 0),
  };
}
