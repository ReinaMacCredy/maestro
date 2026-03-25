/**
 * Greedy token-budget fill utility.
 * Iterates items in order, including each if it fits within the remaining budget.
 */

export function fitWithinBudget<T>(
  items: T[],
  getTokens: (item: T) => number,
  budget: number,
): T[] {
  const result: T[] = [];
  let used = 0;
  for (const item of items) {
    const tokens = getTokens(item);
    if (used + tokens > budget) break;
    result.push(item);
    used += tokens;
  }
  return result;
}
