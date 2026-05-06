/**
 * Concurrency-limited async map. Runs at most `limit` workers in parallel,
 * preserves input order in the result, and surfaces the first error.
 *
 * Use this instead of `Promise.all(items.map(...))` when the per-item work
 * touches a rate-limited resource (GitHub API, fs handle pool, network).
 */
export async function mapWithConcurrency<T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  if (items.length === 0) return [];
  const safeLimit = Math.max(1, Math.min(limit, items.length));
  const results: R[] = new Array(items.length);
  let next = 0;
  async function worker(): Promise<void> {
    while (true) {
      const index = next++;
      if (index >= items.length) return;
      results[index] = await fn(items[index]!, index);
    }
  }
  const workers: Promise<void>[] = [];
  for (let i = 0; i < safeLimit; i++) workers.push(worker());
  await Promise.all(workers);
  return results;
}
